// Typed client for the fluxd API. The agent authenticates with the agent
// token: it can read state, propose plans and apply tier 0-1 changes, but it
// cannot approve anything.

import type { Catalog, Hardware, PlanRecord, ProfileSummary, RawOp, SystemView, ToolInfo } from "./types.js";

export class FluxdError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
    readonly details?: unknown,
  ) {
    super(message);
  }
}

export interface PlanBody {
  intent: { prompt?: string; source?: string; session_id?: string };
  summary?: string;
  rationale?: string;
  engine?: Record<string, unknown>;
  operations: RawOp[];
}

export class Fluxd {
  constructor(
    readonly baseUrl: string,
    private readonly token: string | undefined,
  ) {}

  private async request<T>(method: string, path: string, body?: unknown, timeoutMs = 120_000): Promise<T> {
    const res = await fetch(this.baseUrl + path, {
      method,
      headers: {
        ...(this.token ? { authorization: `Bearer ${this.token}` } : {}),
        ...(body !== undefined ? { "content-type": "application/json" } : {}),
      },
      body: body !== undefined ? JSON.stringify(body) : undefined,
      signal: AbortSignal.timeout(timeoutMs),
    });
    const text = await res.text();
    let data: unknown = undefined;
    try {
      data = text ? JSON.parse(text) : undefined;
    } catch {
      data = text;
    }
    if (!res.ok) {
      const err = (data as { error?: { code?: string; message?: string; details?: unknown } } | undefined)?.error;
      throw new FluxdError(res.status, err?.code ?? "http_error", err?.message ?? `fluxd ${method} ${path} -> ${res.status}`, err?.details);
    }
    return data as T;
  }

  health() {
    return this.request<{ status: string; version: string; generation: number; target: string; ready: boolean; degraded: string[] }>("GET", "/v1/health", undefined, 3000);
  }
  system() {
    return this.request<SystemView>("GET", "/v1/system");
  }
  hardware() {
    return this.request<Hardware>("GET", "/v1/hardware");
  }
  async tools(): Promise<ToolInfo[]> {
    return (await this.request<{ tools: ToolInfo[] }>("GET", "/v1/tools")).tools;
  }
  catalog() {
    return this.request<Catalog>("GET", "/v1/catalog");
  }
  async profiles(): Promise<ProfileSummary[]> {
    return (await this.request<{ profiles: ProfileSummary[] }>("GET", "/v1/profiles")).profiles;
  }
  profile(id: string) {
    return this.request<ProfileSummary & { description: string }>("GET", `/v1/profiles/${encodeURIComponent(id)}`);
  }
  validate(plan: PlanBody) {
    return this.request<PlanRecord>("POST", "/v1/plans/validate", plan);
  }
  propose(plan: PlanBody, autoApply: boolean) {
    return this.request<PlanRecord>("POST", `/v1/plans${autoApply ? "?auto_apply=true" : ""}`, plan, 50 * 60_000);
  }
  rollback(to: number | null, prompt: string) {
    return this.request<PlanRecord>("POST", "/v1/rollback", { to: to ?? undefined, prompt }, 50 * 60_000);
  }
  async generations(): Promise<{ number: number; status: string; summary: string; created_at: string; theme_name: string; parent: number | null }[]> {
    return (await this.request<{ generations: any[] }>("GET", "/v1/generations")).generations;
  }
  async audit(limit = 20): Promise<{ seq: number; ts: string; kind: string; actor: string; summary: string }[]> {
    return (await this.request<{ entries: any[] }>("GET", `/v1/audit?limit=${limit}`)).entries;
  }
}
