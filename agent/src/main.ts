#!/usr/bin/env node
// flux-agent: the AI orchestrator. Runs unprivileged and talks to fluxd with
// the agent token; it can propose plans and apply tier 0-1 changes, never
// approve higher tiers.

import { timingSafeEqual } from "node:crypto";

import { serve } from "@hono/node-server";
import { Hono } from "hono";
import { cors } from "hono/cors";
import { streamSSE } from "hono/streaming";
import { z } from "zod";

import { config } from "./config.js";
import { ContextLoader } from "./context.js";
import { Fluxd } from "./fluxd.js";
import { log } from "./log.js";
import { Compiler } from "./pipeline.js";
import { Router } from "./providers/index.js";
import { loadKnowledge } from "./retrieval.js";
import { Sessions } from "./sessions.js";

const VERSION = "0.1.0";

const IntentBody = z.object({
  prompt: z.string().min(1).max(4000),
  session_id: z.string().max(64).optional(),
  answers: z.record(z.string().max(40), z.string().max(500)).optional(),
  auto_apply: z.boolean().optional(),
  engine: z.enum(["auto", "local", "llm"]).optional(),
});

const fluxd = new Fluxd(config.fluxdUrl, config.fluxdToken);
const router = Router.fromConfig(config.providers);
const knowledge = loadKnowledge();
const loader = new ContextLoader(fluxd, knowledge);
const sessions = new Sessions();
const compiler = new Compiler(config, fluxd, router, loader, sessions);

function authorized(header: string | undefined): boolean {
  if (!config.intentToken) return true;
  const presented = header?.startsWith("Bearer ") ? header.slice(7).trim() : "";
  const a = Buffer.from(presented);
  const b = Buffer.from(config.intentToken);
  return a.length === b.length && timingSafeEqual(a, b);
}

const app = new Hono();
app.use("*", cors({ origin: config.corsOrigins, allowHeaders: ["authorization", "content-type"], allowMethods: ["GET", "POST"], maxAge: 600 }));
app.use("/v1/intents/*", async (c, next) => (authorized(c.req.header("authorization")) ? next() : c.json({ error: { code: "unauthorized", message: "invalid intent token" } }, 401)));
app.use("/v1/intents", async (c, next) => (authorized(c.req.header("authorization")) ? next() : c.json({ error: { code: "unauthorized", message: "invalid intent token" } }, 401)));
app.use("/v1/sessions/*", async (c, next) => (authorized(c.req.header("authorization")) ? next() : c.json({ error: { code: "unauthorized", message: "invalid intent token" } }, 401)));

app.get("/v1/health", async (c) => {
  let core: unknown;
  try {
    core = await fluxd.health();
  } catch (e) {
    core = { status: "unreachable", error: (e as Error).message };
  }
  return c.json({
    status: (core as { status?: string }).status === "unreachable" ? "degraded" : "ok",
    version: VERSION,
    fluxd: core,
    routing: config.routing,
    fast_path_coverage: config.fastPathCoverage,
    knowledge_docs: knowledge.length,
    providers: router.status(),
    local_only: router.providers.every((p) => p.local),
  });
});

app.post("/v1/intents", async (c) => {
  const parsed = IntentBody.safeParse(await c.req.json().catch(() => ({})));
  if (!parsed.success) return c.json({ error: { code: "bad_request", message: parsed.error.issues.map((i) => `${i.path.join(".")}: ${i.message}`).join("; ") } }, 400);
  const result = await compiler.run(parsed.data);
  log.info("intent", { id: result.id, status: result.status, engine: result.engine.kind, provider: result.engine.provider, tier: result.plan?.tier });
  return c.json(result);
});

app.post("/v1/intents/stream", async (c) => {
  const parsed = IntentBody.safeParse(await c.req.json().catch(() => ({})));
  if (!parsed.success) return c.json({ error: { code: "bad_request", message: parsed.error.issues.map((i) => i.message).join("; ") } }, 400);
  return streamSSE(c, async (stream) => {
    let n = 0;
    const result = await compiler.run(parsed.data, (s) => {
      void stream.writeSSE({ event: "step", id: String(++n), data: JSON.stringify(s) });
    });
    await stream.writeSSE({ event: "result", id: String(++n), data: JSON.stringify(result) });
  });
});

app.get("/v1/sessions/:id", (c) => {
  const s = sessions.peek(c.req.param("id"));
  return s ? c.json(s) : c.json({ error: { code: "not_found", message: "unknown session" } }, 404);
});

app.notFound((c) => c.json({ error: { code: "not_found", message: "no such route" } }, 404));
app.onError((e, c) => {
  log.error("unhandled", { error: e.message, stack: e.stack });
  return c.json({ error: { code: "internal", message: e.message } }, 500);
});

async function boot() {
  log.info("flux-agent starting", { version: VERSION, fluxd: config.fluxdUrl, routing: config.routing });
  if (!config.fluxdToken) log.warn("FLUX_AGENT_TOKEN is not set; fluxd will reject requests");
  if (!config.intentToken) log.warn("FLUX_INTENT_TOKEN is not set; /v1/intents is open to any local process (development only)");
  if (!config.intentToken && !["127.0.0.1", "::1", "localhost"].includes(config.host)) {
    throw new Error("refusing to listen on a non-loopback address without FLUX_INTENT_TOKEN");
  }
  log.info("AI providers", {
    chain: router.providers.map((p) => `${p.id}/${p.model}${p.local ? " (local)" : ""}`),
    fallback: "local rule engine",
  });
  log.info("knowledge loaded", { docs: knowledge.length });
  try {
    const h = await fluxd.health();
    log.info("linked to fluxd", { version: h.version, generation: h.generation, target: h.target, status: h.status });
  } catch (e) {
    log.warn("fluxd not reachable yet; will retry per request", { error: (e as Error).message });
  }
  serve({ fetch: app.fetch, hostname: config.host, port: config.port }, (info) => {
    log.info("listening", { url: `http://${info.address}:${info.port}`, routes: ["POST /v1/intents", "POST /v1/intents/stream", "GET /v1/health"] });
  });
}

boot().catch((e) => {
  log.error("boot failed", { error: (e as Error).message });
  process.exit(1);
});
