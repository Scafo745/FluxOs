// Shapes shared with fluxd. fluxd is the source of truth for the schema;
// these types cover the fields the agent reads.

export type Json = null | boolean | number | string | Json[] | { [k: string]: Json };

export interface RawOp {
  tool: string;
  params: Record<string, unknown>;
  reason?: string;
}

export interface Issue {
  severity: "error" | "warning";
  code: string;
  path: string;
  message: string;
  op?: number;
  suggestions?: string[];
}

export interface ChangeView {
  path: string;
  before: unknown;
  after: unknown;
  tier: number;
  summary: string;
  requires_reboot: boolean;
}

export type Decision = "auto_apply" | "confirm" | "admin_approval" | "blocked";

export type PlanStatus =
  | "proposed"
  | "approved"
  | "applied"
  | "staged"
  | "rejected"
  | "rolled_back"
  | "failed"
  | "blocked"
  | "invalid"
  | "no_op";

export interface HealthResult {
  id: string;
  name: string;
  status: "pass" | "warn" | "fail";
  detail: string;
}

export interface PlanRecord {
  id: string;
  status: PlanStatus;
  kind: "change" | "restore";
  decision: Decision;
  plan_hash: string;
  change_summary: string;
  base_generation: number;
  operations: RawOp[];
  evaluation: {
    ok: boolean;
    blocked: boolean;
    tier: number;
    issues: Issue[];
    changes: ChangeView[];
    requires_reboot: boolean;
  };
  result?: {
    generation: number | null;
    status: string;
    message: string;
    health: HealthResult[];
    rolled_back: boolean;
  } | null;
}

export interface ToolInfo {
  name: string;
  title: string;
  description: string;
  tier: number;
  tier_label: string;
  scope: string;
  blocked: boolean;
  blocked_reason?: string;
  params_schema: Record<string, unknown>;
}

export interface PackageEntry {
  id: string;
  name: string;
  category: string;
  description: string;
  nix: { kind: "pkg" | "expr" | "module" | "flatpak-only"; value?: string };
  flatpak: string | null;
  tags: string[];
}

export interface Catalog {
  packages: PackageEntry[];
  services: { id: string; name: string; description: string; tags: string[]; opens_tcp: number[] }[];
  fonts: { family: string; kind: string; notes: string }[];
  icon_themes: { name: string; notes: string }[];
  cursor_themes: { name: string; notes: string }[];
  kernels: { variant: string; label: string; description: string; version_hint: string }[];
  schedulers: { id: string; binary: string | null; description: string; maturity: string; good_for: string[] }[];
  sysctls: { key: string; description: string; rule: Record<string, unknown> }[];
  splash_themes: string[];
}

export interface ProfileSummary {
  id: string;
  name: string;
  version: string;
  vertical: string | null;
  summary: string;
  tags: string[];
  status: string;
  compliance_review_required: boolean;
  theme_name: string;
  desktop: string;
  style: string;
  kernel: string;
}

// The parts of SystemSpec the agent reasons about.
export interface Spec {
  base: { hostname: string; timezone: string; locale: string };
  boot: { splash: boolean; splash_theme: string };
  kernel: { variant: string; sysctl: Record<string, number | string>; cpu_governor: string; io_scheduler: string; sched_ext: string };
  drivers: { gpu: string; gaming_controllers: boolean };
  packages: { system: string[]; flatpak: string[] };
  services: { enabled: string[] };
  security: {
    firewall: { enabled: boolean; allowed_tcp_ports: number[]; allowed_udp_ports: number[] };
    mac: string;
    usb_storage: string;
    screen_lock: { enabled: boolean; idle_minutes: number };
    audit: boolean;
  };
  session: { mode: string; kiosk_url: string | null; ephemeral_home: boolean };
  desktop: {
    environment: string;
    style: string;
    dock: { enabled: boolean; position: string; autohide: boolean; size_px: number; floating: boolean };
    panel: { position: string; global_menu: boolean; height_px: number };
    windows: { snapping: boolean; tiling: boolean; gaps_px: number; corner_radius_px: number; blur: boolean };
    workspaces: number;
    wallpaper: { style: string; seed: number };
  };
  design: {
    theme_name: string;
    icon_theme: string;
    cursor_theme: string;
    tokens: {
      scheme: "dark" | "light";
      palette: Record<string, string>;
      typography: { font_sans: string; font_mono: string; font_display: string; base_size_px: number; line_height: number };
      motion: { animation_scale: number };
      effects: { blur_px: number; transparency: number };
    };
  };
  accessibility: { high_contrast: boolean; large_text: boolean; text_scale: number; screen_reader: boolean; reduce_motion: boolean };
}

export interface SystemView {
  generation: number;
  status: string;
  theme_name: string;
  spec: Spec;
  hardware: string;
  mode: "personal" | "fleet";
  degraded: string[];
  pending_approvals: number;
}

export interface Hardware {
  source: string;
  gpus: { vendor: string }[];
  secure_boot: boolean | null;
  virtualization: string | null;
  memory_mb: number;
  cpu: { model: string; threads: number };
}

/** What the planner (LLM or heuristic) produces. Untrusted until fluxd validates it. */
export interface Draft {
  kind: "change" | "question" | "clarify" | "rollback";
  summary?: string;
  rationale?: string;
  answer?: string;
  questions?: Question[];
  operations: RawOp[];
  rollback_to?: number | null;
}

export interface Question {
  id: string;
  question: string;
  why?: string;
  options?: string[];
}

export interface EngineInfo {
  kind: "heuristic" | "llm" | "cache" | "rollback";
  provider?: string;
  model?: string;
  coverage?: number;
  attempts?: number;
}

export interface TraceStep {
  stage: string;
  detail: string;
  ms: number;
  data?: unknown;
}

export type IntentStatus =
  | "applied"
  | "staged"
  | "awaiting_approval"
  | "needs_clarification"
  | "answered"
  | "blocked"
  | "invalid"
  | "no_op"
  | "rolled_back"
  | "failed"
  | "error";

export interface IntentResult {
  id: string;
  session_id: string;
  status: IntentStatus;
  prompt: string;
  message: string;
  engine: EngineInfo;
  ai_summary?: string;
  rationale?: string;
  answer?: string;
  questions?: Question[];
  plan?: {
    id: string;
    tier: number;
    decision: Decision;
    plan_hash: string;
    change_summary: string;
    changes: ChangeView[];
    requires_reboot: boolean;
    issues: Issue[];
    approve_url: string;
  };
  result?: {
    generation: number | null;
    status: string;
    message: string;
    health: { passed: number; total: number; failed: string[] };
  };
  trace: TraceStep[];
}
