// Clients for fluxd (the trust boundary) and flux-agent (the AI). The UI
// sends approvals straight to fluxd with the paired session token; the
// agent only ever receives the intent token.

import { useSession } from "../state/session";
import { readSse } from "./sse";
import type {
  AgentHealth,
  AuditEntry,
  BootReport,
  Catalog,
  Change,
  GenerationMeta,
  Health,
  IntentResult,
  PairResult,
  PlanPreview,
  PlanRecord,
  ProfileDetail,
  ProfileSummary,
  RawOp,
  SessionInfo,
  Spec,
  SystemView,
  Tier,
  TraceStep,
  UiConfig,
  VerifyReport,
  WhoAmI,
  DesktopApp,
  DirListing,
  FileContent,
  ProcessList,
  UserSession,
} from "./types";

export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
    readonly details?: unknown,
  ) {
    super(message);
  }
}

const FLUXD = (import.meta.env.VITE_FLUXD_URL ?? "").replace(/\/$/, "");

export function fluxdUrl(path: string): string {
  return FLUXD + path;
}

/** flux-agent base: the dev-server proxy, or the address fluxd advertises. */
export function agentBase(): string {
  if (import.meta.env.DEV) return "/agent";
  return (useSession.getState().session?.agentUrl ?? "http://127.0.0.1:7701").replace(/\/$/, "");
}

interface RequestOpts {
  method?: string;
  body?: unknown;
  auth?: boolean;
  signal?: AbortSignal;
}

async function request(path: string, opts: RequestOpts): Promise<Response> {
  const headers: Record<string, string> = {};
  const token = useSession.getState().session?.token;
  if (opts.auth !== false && token) headers.authorization = `Bearer ${token}`;
  if (opts.body !== undefined) headers["content-type"] = "application/json";
  let res: Response;
  try {
    res = await fetch(fluxdUrl(path), {
      method: opts.method ?? (opts.body !== undefined ? "POST" : "GET"),
      headers,
      body: opts.body !== undefined ? JSON.stringify(opts.body) : undefined,
      signal: opts.signal,
    });
  } catch {
    throw new ApiError(0, "unreachable", "FluxOS core (fluxd) is unreachable");
  }
  if (!res.ok) {
    let err: { code?: string; message?: string; details?: unknown } | undefined;
    try {
      err = ((await res.json()) as { error?: typeof err }).error;
    } catch {
      err = undefined;
    }
    if (res.status === 401 && opts.auth !== false) useSession.getState().expire();
    throw new ApiError(res.status, err?.code ?? "http_error", err?.message ?? `HTTP ${res.status}`, err?.details);
  }
  return res;
}

async function json<T>(path: string, opts: RequestOpts = {}): Promise<T> {
  return (await (await request(path, opts)).json()) as T;
}

async function text(path: string, opts: RequestOpts = {}): Promise<string> {
  return (await request(path, opts)).text();
}

const enc = encodeURIComponent;

export const api = {
  health: () => json<Health>("/v1/health", { auth: false }),
  boot: () => json<BootReport>("/v1/boot", { auth: false }),
  uiConfig: () => json<UiConfig>("/v1/ui-config", { auth: false }),
  themeCss: () => text("/v1/theme.css", { auth: false }),
  pair: (code: string, name: string) => json<PairResult>("/v1/pair", { auth: false, body: { code, name } }),
  logout: () => json<{ revoked: SessionInfo }>("/v1/logout", { body: {} }),
  whoami: () => json<WhoAmI>("/v1/whoami"),
  system: () => json<SystemView>("/v1/system"),
  catalog: () => json<Catalog>("/v1/catalog"),
  plans: (limit = 60) => json<{ plans: PlanRecord[] }>(`/v1/plans?limit=${limit}`).then((r) => r.plans),
  plan: (id: string) => json<PlanRecord>(`/v1/plans/${enc(id)}`),
  propose: (body: PlanBody, autoApply: boolean) =>
    json<PlanRecord>(`/v1/plans${autoApply ? "?auto_apply=true" : ""}`, { body }),
  validate: (body: PlanBody) => json<PlanRecord>("/v1/plans/validate", { body }),
  approve: (id: string, planHash: string) =>
    json<PlanRecord>(`/v1/plans/${enc(id)}/approve`, { body: { plan_hash: planHash, apply: true } }),
  reject: (id: string, reason?: string) => json<PlanRecord>(`/v1/plans/${enc(id)}/reject`, { body: { reason } }),
  planPreview: (id: string) => json<PlanPreview>(`/v1/plans/${enc(id)}/preview`),
  planPreviewFile: (id: string, path: string) => text(`/v1/plans/${enc(id)}/preview?path=${enc(path)}`),
  generations: () => json<{ current: number; generations: GenerationMeta[] }>("/v1/generations"),
  generation: (n: number) => json<{ meta: GenerationMeta; spec: Spec }>(`/v1/generations/${n}`),
  generationFile: (n: number, path: string) => text(`/v1/generations/${n}/file?path=${enc(path)}`),
  rollback: (to?: number, prompt?: string, sessionId?: string) =>
    json<PlanRecord>("/v1/rollback", {
      body: { ...(to === undefined ? {} : { to }), ...(prompt ? { prompt } : {}), ...(sessionId ? { session_id: sessionId } : {}) },
    }),
  reboot: () => json<{ generation: number; message: string; staged_activated: boolean }>("/v1/system/reboot", { body: {} }),
  poweroff: () => json<{ generation: number; message: string }>("/v1/system/poweroff", { body: {} }),
  profiles: () => json<{ profiles: ProfileSummary[] }>("/v1/profiles").then((r) => r.profiles),
  profile: (id: string) => json<ProfileDetail>(`/v1/profiles/${enc(id)}`),
  saveProfile: (id: string, name: string, summary?: string) =>
    json<ProfileSummary>("/v1/profiles", { body: { id, name, summary } }),
  exportProfile: (id: string) => json<unknown>(`/v1/profiles/${enc(id)}/export`),
  importProfile: (bundle: unknown) => json<ProfileSummary>("/v1/profiles/import", { body: bundle }),
  audit: (opts: { before?: number; limit?: number; kind?: string } = {}) => {
    const q = new URLSearchParams();
    if (opts.before) q.set("before", String(opts.before));
    q.set("limit", String(opts.limit ?? 60));
    if (opts.kind) q.set("kind", opts.kind);
    return json<{ head: [number, string]; entries: AuditEntry[] }>(`/v1/audit?${q}`);
  },
  auditVerify: () => json<VerifyReport>("/v1/audit/verify"),
  sessions: () => json<{ current: string | null; sessions: SessionInfo[] }>("/v1/sessions"),
  revokeSession: (id: string) => json<{ revoked: SessionInfo }>(`/v1/sessions/${enc(id)}/revoke`, { body: {} }),

  // The console user's session (flux-session, relayed by fluxd): the user's own rights, never fluxd's.
  userSession: () => json<UserSession>("/v1/session"),
  files: (path: string) => json<DirListing>(`/v1/session/files?path=${enc(path)}`),
  file: (path: string) => json<FileContent>(`/v1/session/file?path=${enc(path)}`),
  processes: () => json<ProcessList>("/v1/session/processes"),
  signal: (pid: number, signal: "TERM" | "KILL" | "STOP" | "CONT") =>
    json<{ message: string }>(`/v1/session/processes/${pid}/signal`, { body: { signal } }),
  desktopApps: () => json<{ apps: DesktopApp[] }>("/v1/session/apps").then((r) => r.apps),
  launch: (id: string) => json<{ pid: number }>(`/v1/session/apps/${enc(id)}/launch`, { body: {} }),
  terminalTicket: (cols: number, rows: number) => json<{ ticket: string }>("/v1/session/terminal", { body: { cols, rows } }),
};

/** WebSocket URL of a terminal, authorized by a single-use ticket. */
export function terminalSocketUrl(ticket: string): string {
  const u = new URL(fluxdUrl(`/v1/session/terminal?ticket=${enc(ticket)}`), window.location.href);
  u.protocol = u.protocol === "https:" ? "wss:" : "ws:";
  return u.toString();
}

export interface PlanBody {
  intent: { prompt?: string; source?: string; session_id?: string };
  summary?: string;
  engine?: Record<string, unknown>;
  operations: RawOp[];
}

/** What a restore would change, from fluxd's dry-run validation. */
export async function restoreImpact(generation: number, sessionId: string): Promise<{ tier: Tier; changes: Change[]; ok: boolean; error?: string }> {
  const r = await api.validate({
    intent: { prompt: `Restore generation ${generation}`, source: "ui", session_id: sessionId },
    operations: [{ tool: "generation.restore", params: { generation } }],
  });
  const error = r.evaluation.issues.find((i) => i.severity === "error")?.message;
  return { tier: r.evaluation.tier, changes: r.evaluation.changes, ok: r.evaluation.ok, error };
}

// ---------------------------------------------------------------- agent --

function agentHeaders(): Record<string, string> {
  const t = useSession.getState().session?.intentToken;
  return t ? { authorization: `Bearer ${t}` } : {};
}

export async function agentHealth(): Promise<AgentHealth> {
  const res = await fetch(`${agentBase()}/v1/health`);
  if (!res.ok) throw new ApiError(res.status, "agent_error", `flux-agent answered ${res.status}`);
  return (await res.json()) as AgentHealth;
}

/** Runs a natural-language request, reporting each pipeline step as it happens. */
export async function streamIntent(
  body: { prompt: string; session_id: string; answers?: Record<string, string> },
  onStep: (s: TraceStep) => void,
  signal?: AbortSignal,
): Promise<IntentResult> {
  let res: Response;
  try {
    res = await fetch(`${agentBase()}/v1/intents/stream`, {
      method: "POST",
      headers: { "content-type": "application/json", ...agentHeaders() },
      body: JSON.stringify({ ...body, auto_apply: true }),
      signal,
    });
  } catch {
    throw new ApiError(0, "unreachable", "The FluxOS AI (flux-agent) is unreachable");
  }
  if (!res.ok) {
    let message = `flux-agent answered ${res.status}`;
    try {
      message = ((await res.json()) as { error?: { message?: string } }).error?.message ?? message;
    } catch {
      /* not JSON */
    }
    throw new ApiError(res.status, "agent_error", message);
  }
  let result: IntentResult | null = null;
  await readSse(res, (m) => {
    if (m.event === "step") onStep(JSON.parse(m.data) as TraceStep);
    else if (m.event === "result") result = JSON.parse(m.data) as IntentResult;
  });
  if (!result) throw new ApiError(0, "agent_error", "the AI stream ended without a result");
  return result;
}
