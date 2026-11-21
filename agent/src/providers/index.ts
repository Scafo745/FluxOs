// Multi-provider router with ordered failover and a per-provider circuit
// breaker, so one slow or broken provider does not stall every request.

import type { ProviderConfig } from "../config.js";
import { log } from "../log.js";
import { AnthropicProvider } from "./anthropic.js";
import { OllamaProvider } from "./ollama.js";
import { OpenAIProvider } from "./openai.js";
import { type CompletionRequest, type CompletionResult, type Provider, ProviderError } from "./types.js";

export * from "./types.js";

interface Health {
  failures: number[];
  openUntil: number;
  lastError?: string;
  lastOkMs?: number;
}

export interface Attempt {
  provider: string;
  model: string;
  ok: boolean;
  ms: number;
  error?: string;
}

const WINDOW_MS = 120_000;
const TRIP_AFTER = 3;
const OPEN_MS = 60_000;

export class Router {
  private readonly health = new Map<string, Health>();

  constructor(readonly providers: Provider[]) {
    for (const p of providers) this.health.set(p.id, { failures: [], openUntil: 0 });
  }

  static fromConfig(cfgs: ProviderConfig[]): Router {
    return new Router(
      cfgs.map((c) => {
        switch (c.id) {
          case "anthropic":
            return new AnthropicProvider(c);
          case "openai":
            return new OpenAIProvider(c);
          case "ollama":
            return new OllamaProvider(c);
        }
      }),
    );
  }

  get available(): boolean {
    return this.providers.length > 0;
  }

  status() {
    const now = Date.now();
    return this.providers.map((p) => {
      const h = this.health.get(p.id)!;
      return {
        id: p.id,
        model: p.model,
        local: p.local,
        circuit: h.openUntil > now ? "open" : "closed",
        recent_failures: h.failures.filter((t) => now - t < WINDOW_MS).length,
        last_error: h.lastError,
        last_latency_ms: h.lastOkMs,
      };
    });
  }

  private fail(id: string, err: string) {
    const h = this.health.get(id)!;
    const now = Date.now();
    h.failures = [...h.failures.filter((t) => now - t < WINDOW_MS), now];
    h.lastError = err;
    if (h.failures.length >= TRIP_AFTER) {
      h.openUntil = now + OPEN_MS;
      log.warn("provider circuit opened", { provider: id, for_ms: OPEN_MS });
    }
  }

  /** Tries providers in order until one answers. */
  async complete(req: CompletionRequest, timeoutMs: number, onAttempt?: (a: Attempt) => void): Promise<{ result: CompletionResult; attempts: Attempt[] }> {
    const attempts: Attempt[] = [];
    const now = Date.now();
    for (const p of this.providers) {
      const h = this.health.get(p.id)!;
      if (h.openUntil > now) {
        const a = { provider: p.id, model: p.model, ok: false, ms: 0, error: "circuit open (recent failures)" };
        attempts.push(a);
        onAttempt?.(a);
        continue;
      }
      const started = Date.now();
      try {
        const result = await p.complete(req, AbortSignal.timeout(timeoutMs));
        h.failures = [];
        h.lastOkMs = result.ms;
        const a = { provider: p.id, model: result.model, ok: true, ms: result.ms };
        attempts.push(a);
        onAttempt?.(a);
        return { result, attempts };
      } catch (e) {
        const msg = e instanceof ProviderError ? e.message : `${p.id}: ${(e as Error).message}`;
        this.fail(p.id, msg);
        const a = { provider: p.id, model: p.model, ok: false, ms: Date.now() - started, error: msg };
        attempts.push(a);
        onAttempt?.(a);
        log.warn("provider failed, trying next", { provider: p.id, error: msg });
      }
    }
    throw new Error(`all AI providers failed: ${attempts.map((a) => a.error).join(" | ") || "none configured"}`);
  }
}
