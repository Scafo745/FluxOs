// The profile compiler (PRD section 6): intake, retrieve, generate,
// validate, preview, approve-or-apply, verify. The AI sits in the middle;
// fluxd is the trust boundary on both sides of it.

import { z } from "zod";

import type { Config } from "./config.js";
import type { Context, ContextLoader } from "./context.js";
import { type Fluxd, FluxdError, type PlanBody } from "./fluxd.js";
import { heuristic } from "./heuristic.js";
import { log } from "./log.js";
import { DRAFT_SCHEMA, repairMessage, systemPrompt, userMessage } from "./prompt.js";
import type { ChatMessage, Router } from "./providers/index.js";
import { extractJson } from "./providers/json.js";
import { PlanCache, type Session, type Sessions } from "./sessions.js";
import type { Draft, EngineInfo, IntentResult, IntentStatus, PlanRecord, TraceStep } from "./types.js";

const DraftSchema = z.object({
  kind: z.enum(["change", "question", "clarify", "rollback"]),
  summary: z.string().max(500).optional(),
  rationale: z.string().max(2000).optional(),
  answer: z.string().max(4000).optional(),
  questions: z
    .array(z.object({ id: z.string().max(40), question: z.string().max(300), why: z.string().max(300).optional(), options: z.array(z.string().max(120)).max(6).optional() }))
    .max(3)
    .optional(),
  rollback_to: z.number().int().nullable().optional(),
  operations: z
    .array(z.object({ tool: z.string().max(64), params: z.record(z.string(), z.unknown()).default({}), reason: z.string().max(300).optional() }))
    .max(40)
    .default([]),
});

export interface IntentRequest {
  prompt: string;
  session_id?: string;
  answers?: Record<string, string>;
  /** Apply tier 0-1 plans immediately (default true). */
  auto_apply?: boolean;
  /** Force an engine: auto (default), local (rules only) or llm. */
  engine?: "auto" | "local" | "llm";
}

export type Emit = (step: TraceStep) => void;

function sanitize(prompt: string): string {
  // eslint-disable-next-line no-control-regex
  return prompt.replace(/[\u0000-\u0008\u000b-\u001f\u007f]/g, " ").trim().slice(0, 2000);
}

function statusOf(plan: PlanRecord): IntentStatus {
  switch (plan.status) {
    case "applied":
      return "applied";
    case "staged":
      return "staged";
    case "proposed":
    case "approved":
      return "awaiting_approval";
    case "blocked":
      return "blocked";
    case "invalid":
      return "invalid";
    case "no_op":
      return "no_op";
    case "rolled_back":
      return "rolled_back";
    default:
      return "failed";
  }
}

function tierLabel(t: number): string {
  return ["safe", "low", "medium", "high", "blocked"][t] ?? "unknown";
}

export class Compiler {
  private readonly cache = new PlanCache();

  constructor(
    private readonly cfg: Config,
    private readonly fluxd: Fluxd,
    private readonly router: Router,
    private readonly loader: ContextLoader,
    private readonly sessions: Sessions,
  ) {}

  async run(req: IntentRequest, emit: Emit = () => {}): Promise<IntentResult> {
    const t0 = Date.now();
    const trace: TraceStep[] = [];
    const step = (stage: string, detail: string, data?: unknown) => {
      const s = { stage, detail, ms: Date.now() - t0, data };
      trace.push(s);
      emit(s);
    };
    const prompt = sanitize(req.prompt ?? "");
    const session = this.sessions.get(req.session_id);
    const answers = { ...(req.answers ?? {}) };
    const id = `in_${crypto.randomUUID().replace(/-/g, "").slice(0, 24)}`;
    const base = (status: IntentStatus, message: string, engine: EngineInfo, extra: Partial<IntentResult> = {}): IntentResult => {
      const result: IntentResult = { id, session_id: session.id, status, prompt, message, engine, trace, ...extra };
      this.sessions.record(session, {
        at: new Date().toISOString(),
        prompt,
        status,
        outcome: `${status}: ${message.slice(0, 200)}`,
        plan_id: extra.plan?.id,
        generation: extra.result?.generation,
      });
      session.pending = status === "needs_clarification" ? { prompt, questions: extra.questions ?? [] } : undefined;
      step("done", `${status}: ${message.split("\n")[0]}`);
      return result;
    };

    if (!prompt) return base("error", "Tell me what you would like to change.", { kind: "heuristic" });
    step("intake", prompt);

    let ctx: Context;
    try {
      ctx = await this.loader.load();
    } catch (e) {
      return base("error", `FluxOS core (fluxd) is unreachable: ${(e as Error).message}`, { kind: "heuristic" });
    }
    step("context", `generation ${ctx.system.generation} · ${ctx.system.theme_name} · ${ctx.system.spec.desktop.environment}`, {
      generation: ctx.system.generation,
      hardware: ctx.system.hardware,
    });

    const refs = ctx.index.search(`${prompt} ${Object.values(answers).join(" ")}`, 6);
    step("retrieve", refs.length ? refs.map((r) => r.doc.title).join(" · ") : "no matching references", refs.map((r) => ({ title: r.doc.title, kind: r.doc.kind, score: Math.round(r.score * 100) / 100 })));

    // ---- route: local fast path first, LLM for novel requests
    const mode = req.engine ?? this.cfg.routing;
    const h = mode === "llm" && this.router.available ? null : heuristic(prompt, ctx, answers, this.cfg.fastPathCoverage);
    let draft: Draft | undefined;
    let engine: EngineInfo = { kind: "heuristic" };
    const cacheKey = PlanCache.key(prompt, answers, ctx.system.spec_digest);
    const useLocal = !!h && (mode === "local" || h.confident || !this.router.available);
    if (useLocal) {
      draft = h!.draft;
      engine = { kind: "heuristic", coverage: h!.coverage };
      step("route", `local rule engine (understood ${Math.round(h!.coverage * 100)}% of the request${this.router.available ? "" : "; no AI provider configured"})`, { rules: h!.rules });
    } else if (mode === "local") {
      return base("error", "The local rule engine did not recognize this request. Try a more specific phrasing or enable an AI provider.", engine);
    } else if (!this.router.available) {
      return base(
        "error",
        "I could not map this request with the local rules, and no AI provider is configured. Set ANTHROPIC_API_KEY, OPENAI_API_KEY or OLLAMA_HOST for open-ended requests.",
        engine,
      );
    } else {
      const cached = this.cache.get(cacheKey);
      if (cached) {
        draft = cached.draft;
        engine = { kind: "cache", ...cached.engine };
        step("route", `cached plan from ${cached.engine.provider}/${cached.engine.model}`);
      } else {
        step("route", h ? `LLM (local rules understood only ${Math.round(h.coverage * 100)}%)` : "LLM (novel request)");
      }
    }

    const messages: ChatMessage[] = [{ role: "user", content: userMessage(prompt, answers, this.sessions.history(session)) }];
    const system = useLocal || draft ? "" : systemPrompt(ctx, refs.map((r) => ({ title: r.doc.title, text: r.doc.text })));
    let attempts = 0;
    const generate = async (): Promise<Draft> => {
      attempts++;
      const { result, attempts: tries } = await this.router.complete(
        { system, messages, schema: DRAFT_SCHEMA as unknown as Record<string, unknown>, maxTokens: 4096 },
        this.cfg.llmTimeoutMs,
        (a) => step("provider", a.ok ? `${a.provider}/${a.model} answered in ${a.ms} ms` : `${a.provider} failed: ${a.error}`, a),
      );
      engine = { kind: "llm", provider: result.provider, model: result.model, attempts: tries.length };
      messages.push({ role: "assistant", content: result.text });
      let parsed: unknown;
      try {
        parsed = extractJson(result.text);
      } catch (e) {
        throw new Error(`model output was not JSON: ${(e as Error).message}`);
      }
      const r = DraftSchema.safeParse(parsed);
      if (!r.success) throw new Error(`model output did not match the plan schema: ${r.error.issues.map((i) => `${i.path.join(".")}: ${i.message}`).join("; ")}`);
      return r.data as Draft;
    };

    if (!draft) {
      try {
        draft = await generate();
        step("generate", `${engine.provider}/${engine.model}: ${draft.kind}${draft.operations.length ? ` with ${draft.operations.length} operations` : ""}`, { summary: draft.summary });
      } catch (e) {
        // Structural failure: give the model one more chance, then fall back to local rules.
        try {
          messages.push({ role: "user", content: `${(e as Error).message}. Reply with only the JSON object described in the instructions.` });
          draft = await generate();
          step("generate", `${engine.provider}/${engine.model} (retry): ${draft.kind}`);
        } catch (e2) {
          log.warn("LLM generation failed", { error: (e2 as Error).message });
          if (h && h.draft.operations.length + (h.draft.kind !== "change" ? 1 : 0) > 0) {
            draft = h.draft;
            engine = { kind: "heuristic", coverage: h.coverage };
            step("route", `AI providers failed (${(e2 as Error).message}); using the local rule engine`);
          } else {
            return base("error", `The AI planner failed: ${(e2 as Error).message}`, engine);
          }
        }
      }
    }

    // ---- non-change outcomes
    if (draft.kind === "clarify" && draft.questions?.length) {
      const unanswered = draft.questions.filter((q) => answers[q.id] === undefined);
      if (unanswered.length) {
        step("clarify", unanswered.map((q) => q.question).join(" | "));
        return base("needs_clarification", unanswered.map((q) => q.question).join("\n"), engine, { questions: unanswered, ai_summary: draft.summary });
      }
    }
    if (draft.kind === "question") {
      let answer = draft.answer ?? "I don't know yet.";
      if (answer === "history") {
        const gens = await this.fluxd.generations();
        answer = gens
          .slice(0, 6)
          .map((g) => `Generation ${g.number} (${g.status}, ${g.created_at.slice(0, 16).replace("T", " ")}): ${g.summary.slice(0, 140)}`)
          .join("\n");
      }
      return base("answered", answer, engine, { answer });
    }
    if (draft.kind === "rollback") {
      try {
        const plan = await this.fluxd.rollback(draft.rollback_to ?? null, prompt, session.id);
        step("rollback", `${plan.status}: ${plan.change_summary.split("\n")[0]}`);
        return this.finish(base, plan, engine, draft, ctx);
      } catch (e) {
        return base("error", e instanceof FluxdError ? e.message : `rollback failed: ${(e as Error).message}`, { kind: "rollback" });
      }
    }
    if (!draft.operations.length) {
      return base("no_op", draft.answer ?? draft.summary ?? "Nothing to change.", engine);
    }

    // ---- validate (with repair loop for LLM plans)
    const planBody = (d: Draft): PlanBody => ({
      intent: { prompt, source: "agent", session_id: session.id },
      summary: d.summary,
      rationale: d.rationale,
      engine: { ...engine },
      operations: d.operations,
    });
    let check = await this.fluxd.validate(planBody(draft));
    step("validate", check.evaluation.ok ? `valid · tier ${check.evaluation.tier} (${tierLabel(check.evaluation.tier)}) · ${check.evaluation.changes.length} changes` : `rejected: ${check.evaluation.issues.filter((i) => i.severity === "error").map((i) => i.message).join("; ")}`, {
      ok: check.evaluation.ok,
      tier: check.evaluation.tier,
    });
    let repairs = 0;
    while (!check.evaluation.ok && !check.evaluation.blocked && this.router.available && repairs < this.cfg.maxRepairs) {
      repairs++;
      if (engine.kind !== "llm") {
        // Hand a failed local plan to the LLM together with fluxd's feedback.
        messages.length = 0;
        messages.push({ role: "user", content: userMessage(prompt, answers, this.sessions.history(session)) });
        messages.push({ role: "assistant", content: JSON.stringify(draft) });
      }
      messages.push({ role: "user", content: repairMessage(check.evaluation.issues) });
      try {
        const sys = systemPrompt(ctx, refs.map((r) => ({ title: r.doc.title, text: r.doc.text })));
        const { result } = await this.router.complete({ system: sys, messages, schema: DRAFT_SCHEMA as unknown as Record<string, unknown>, maxTokens: 4096 }, this.cfg.llmTimeoutMs);
        engine = { kind: "llm", provider: result.provider, model: result.model, attempts: attempts + repairs };
        messages.push({ role: "assistant", content: result.text });
        const r = DraftSchema.safeParse(extractJson(result.text));
        if (!r.success) break;
        draft = r.data as Draft;
      } catch (e) {
        step("repair", `repair attempt ${repairs} failed: ${(e as Error).message}`);
        break;
      }
      check = await this.fluxd.validate(planBody(draft));
      step("repair", check.evaluation.ok ? `repair ${repairs}: valid · tier ${check.evaluation.tier}` : `repair ${repairs}: still invalid`);
    }

    if (engine.kind === "llm" && check.evaluation.ok) this.cache.set(cacheKey, { draft, engine: { provider: engine.provider, model: engine.model } });

    // ---- propose (always recorded and audited, even when refused) and auto-apply tier 0-1
    let plan: PlanRecord;
    try {
      plan = await this.fluxd.propose(planBody(draft), req.auto_apply !== false);
    } catch (e) {
      return base("error", e instanceof FluxdError ? `fluxd refused: ${e.message}` : `fluxd error: ${(e as Error).message}`, engine);
    }
    step("propose", `${plan.id} · ${plan.status} · tier ${plan.evaluation.tier} · decision ${plan.decision}`, { plan_id: plan.id, decision: plan.decision });
    if (plan.result) {
      const passed = plan.result.health.filter((x) => x.status === "pass").length;
      step("verify", `${plan.result.status}: ${passed}/${plan.result.health.length} health checks passed`, { generation: plan.result.generation });
    }
    if (plan.status === "applied" || plan.status === "staged" || plan.status === "rolled_back") this.loader.invalidate();
    return this.finish(base, plan, engine, draft, ctx);
  }

  private finish(
    base: (s: IntentStatus, m: string, e: EngineInfo, x?: Partial<IntentResult>) => IntentResult,
    plan: PlanRecord,
    engine: EngineInfo,
    draft: Draft,
    ctx: Context,
  ): IntentResult {
    const status = statusOf(plan);
    const ev = plan.evaluation;
    const errors = ev.issues.filter((i) => i.severity === "error");
    const health = plan.result?.health ?? [];
    const changes = plan.change_summary;
    let message: string;
    switch (status) {
      case "applied":
        message = `Done: generation ${plan.result?.generation} is live.\n${changes}\nSay "undo" to go back.`;
        break;
      case "staged":
        message = `Installed as generation ${plan.result?.generation}. It takes effect after a reboot; the previous version stays in the boot menu.\n${changes}`;
        break;
      case "awaiting_approval":
        message = `This is a tier ${ev.tier} (${tierLabel(ev.tier)}) change, so it needs ${plan.decision === "admin_approval" ? "an administrator's" : "your"} approval before anything happens.${ev.requires_reboot ? " It needs a reboot to take effect." : ""}\n${changes}`;
        break;
      case "blocked":
        message = `FluxOS will not do that: ${errors.map((e) => e.message).join(" ")}${draft.rationale && engine.kind === "llm" ? `\n${draft.rationale}` : ""}`;
        break;
      case "invalid":
        message = `I could not build a valid plan: ${errors.map((e) => e.message).join("; ")}`;
        break;
      case "no_op":
        message = "The system already looks like that; nothing changed.";
        break;
      case "rolled_back":
        message = `${plan.result?.message ?? "Health checks failed and the change was rolled back."}`;
        break;
      default:
        message = plan.result?.message ?? `Apply failed (${plan.status}).`;
    }
    const preview = ctx.profiles.find((p) => plan.operations.some((o) => o.tool === "profile.apply" && o.params.profile === p.id && p.compliance_review_required));
    if (preview && status !== "blocked") message += `\nNote: the ${preview.name} profile is a preview. Have your compliance team review it before production use.`;
    return base(status, message, engine, {
      ai_summary: draft.summary,
      rationale: draft.rationale,
      plan: {
        id: plan.id,
        tier: ev.tier,
        decision: plan.decision,
        plan_hash: plan.plan_hash,
        change_summary: changes,
        changes: ev.changes,
        requires_reboot: ev.requires_reboot,
        issues: ev.issues,
        approve_url: `${this.fluxd.baseUrl}/v1/plans/${plan.id}/approve`,
      },
      result: plan.result
        ? {
            generation: plan.result.generation,
            status: plan.result.status,
            message: plan.result.message,
            health: {
              passed: health.filter((x) => x.status === "pass").length,
              total: health.length,
              failed: health.filter((x) => x.status === "fail").map((x) => `${x.name}: ${x.detail}`),
            },
          }
        : undefined,
    });
  }
}
