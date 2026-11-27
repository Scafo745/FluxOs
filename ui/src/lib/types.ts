// Shapes returned by fluxd and flux-agent (see docs/api.md). fluxd is the
// source of truth; these cover the fields the UI reads.

export type Tier = 0 | 1 | 2 | 3 | 4;
export type Mode = "personal" | "fleet";
export type Role = "admin" | "user" | "agent";
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
export type GenStatus = "pending" | "active" | "superseded" | "staged" | "failed" | "rolled-back";
export type StylePreset = "glass-dock" | "snap-layouts" | "tiling" | "classic-panel" | "ops-center" | "focus" | "custom";
export type DesktopEnv = "fluxos" | "plasma" | "gnome" | "hyprland" | "cosmic";

export interface BootStage {
  id: string;
  title: string;
  status: "ok" | "warn" | "fail";
  detail: string;
  duration_ms: number;
  reboot?: boolean;
}

export interface BootReport {
  version: string;
  boot_id: string;
  started_at: string;
  ready: boolean;
  duration_ms: number;
  target: string;
  generation: number;
  stages: BootStage[];
}

export interface Health {
  status: "ok" | "degraded";
  version: string;
  boot_id: string;
  generation: number;
  target: string;
  mode: Mode;
  uptime_s: number;
  degraded: string[];
  ready: boolean;
  event_seq: number;
}

export interface UiConfig {
  version: string;
  boot_id: string;
  mode: Mode;
  agent_url: string;
  pairing: string;
}

export interface Principal {
  id: string;
  role: Role;
  session?: string;
}

export interface SessionInfo {
  id: string;
  name: string;
  role: Role;
  created_at: string;
}

export interface PairResult {
  token: string;
  session: SessionInfo;
  principal: Principal;
  mode: Mode;
  intent_token: string;
  agent_url: string;
}

export interface WhoAmI {
  id: string;
  role: Role;
  session?: string | null;
  mode: Mode;
  can_approve_tiers: number[];
}

export interface Palette {
  background: string;
  surface: string;
  surface_raised: string;
  border: string;
  text: string;
  text_muted: string;
  accent: string;
  accent_text: string;
  secondary: string;
  success: string;
  warning: string;
  danger: string;
}

export interface Spec {
  base: { distro: string; release: string; hostname: string; timezone: string; locale: string };
  boot: { splash: boolean; splash_theme: string; loader_timeout_s: number };
  kernel: {
    variant: string;
    sysctl: Record<string, number | string>;
    cpu_governor: string;
    io_scheduler: string;
    sched_ext: string;
  };
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
  session: { mode: "standard" | "kiosk" | "exam"; kiosk_url: string | null; ephemeral_home: boolean };
  desktop: {
    environment: DesktopEnv;
    style: StylePreset;
    dock: { enabled: boolean; position: "bottom" | "left" | "right"; autohide: boolean; size_px: number; floating: boolean };
    panel: { position: "top" | "bottom"; global_menu: boolean; height_px: number };
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
      palette: Palette;
      typography: {
        font_sans: string;
        font_mono: string;
        font_display: string;
        base_size_px: number;
        scale_ratio: number;
        line_height: number;
        weight_regular: number;
        weight_bold: number;
      };
      shape: { radius_sm_px: number; radius_md_px: number; radius_lg_px: number; spacing_unit_px: number; border_width_px: number };
      motion: {
        duration_fast_ms: number;
        duration_base_ms: number;
        duration_slow_ms: number;
        easing: [number, number, number, number];
        animation_scale: number;
      };
      effects: { blur_px: number; transparency: number; shadow_opacity: number };
    };
  };
  accessibility: { high_contrast: boolean; large_text: boolean; text_scale: number; screen_reader: boolean; reduce_motion: boolean };
}

export interface SystemView {
  generation: number;
  status: GenStatus;
  theme_name: string;
  spec_digest: string;
  spec: Spec;
  target: { kind: string; [k: string]: unknown };
  mode: Mode;
  hardware: string;
  degraded: string[];
  pending_approvals: number;
  audit_head: [number, string];
}

export interface Issue {
  severity: "error" | "warning";
  code: string;
  path: string;
  message: string;
  op?: number;
  suggestions?: string[];
}

export interface Change {
  path: string;
  before: unknown;
  after: unknown;
  tier: Tier;
  summary: string;
  requires_reboot: boolean;
}

export interface HealthCheck {
  id: string;
  name: string;
  status: "pass" | "warn" | "fail";
  detail: string;
}

export interface RawOp {
  tool: string;
  params: Record<string, unknown>;
  reason?: string;
}

export interface ArtifactInfo {
  path: string;
  sha256: string;
  bytes: number;
}

export interface PlanRecord {
  id: string;
  created_at: string;
  updated_at: string;
  status: PlanStatus;
  kind: "change" | "restore";
  proposed_by: string;
  intent: { prompt?: string | null; source?: string | null; session_id?: string | null };
  engine: Record<string, unknown> | null;
  ai_summary: string | null;
  ai_rationale: string | null;
  operations: RawOp[];
  base_generation: number;
  evaluation: {
    ok: boolean;
    blocked: boolean;
    tier: Tier;
    issues: Issue[];
    changes: Change[];
    requires_reboot: boolean;
  };
  decision: Decision;
  change_summary: string;
  plan_hash: string;
  approval: { by: string; role: string; at: string; plan_hash: string } | null;
  preview: { ok: boolean; artifacts: ArtifactInfo[]; checks: HealthCheck[] } | null;
  result: {
    generation: number | null;
    status: string;
    message: string;
    health: HealthCheck[];
    rolled_back: boolean;
    steps: string[];
    duration_ms: number;
  } | null;
}

export interface PlanPreview {
  plan_id: string;
  generation: number;
  spec: Spec;
  artifacts: ArtifactInfo[];
}

export interface GenerationMeta {
  number: number;
  parent: number | null;
  created_at: string;
  plan_id: string | null;
  summary: string;
  tier: Tier;
  status: GenStatus;
  spec_digest: string;
  actor: string;
  requires_reboot: boolean;
  theme_name: string;
  artifacts: ArtifactInfo[];
}

export interface AuditEntry {
  seq: number;
  ts: string;
  kind: string;
  actor: string;
  plan_id?: string;
  generation?: number;
  tier?: number;
  summary: string;
  details: unknown;
  prev_hash: string;
  hash: string;
}

export interface VerifyReport {
  ok: boolean;
  entries: number;
  head: string;
  broken_at?: number;
  error?: string;
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
  integrations: string[];
  source: "builtin" | "user" | "imported";
  theme_name: string;
  desktop: DesktopEnv;
  style: StylePreset;
  kernel: string;
  accent: string;
}

export interface ProfileDetail extends ProfileSummary {
  description: string;
  spec: Spec;
}

export interface CatalogPackage {
  id: string;
  name: string;
  category: string;
  description: string;
  nix: { kind: string; value?: string };
  flatpak: string | null;
  tags: string[];
}

export interface Catalog {
  packages: CatalogPackage[];
  services: { id: string; name: string; description: string; opens_tcp: number[]; tags: string[] }[];
  kernels: { variant: string; label: string; description: string; version_hint: string }[];
  schedulers: { id: string; binary: string | null; description: string; maturity: string; good_for: string[] }[];
  fonts: { family: string; kind: string; notes: string }[];
}

export interface FluxEvent {
  seq: number;
  ts: string;
  kind: string;
  // Event payloads differ per kind; see docs/api.md.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  data: Record<string, any>;
}

// ---------------------------------------------------------------- agent --

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

export interface TraceStep {
  stage: string;
  detail: string;
  ms: number;
  data?: unknown;
}

export interface Question {
  id: string;
  question: string;
  why?: string;
  options?: string[];
}

export interface IntentResult {
  id: string;
  session_id: string;
  status: IntentStatus;
  prompt: string;
  message: string;
  engine: { kind: string; provider?: string; model?: string; coverage?: number; attempts?: number };
  ai_summary?: string;
  rationale?: string;
  answer?: string;
  questions?: Question[];
  plan?: {
    id: string;
    tier: Tier;
    decision: Decision;
    plan_hash: string;
    change_summary: string;
    changes: Change[];
    requires_reboot: boolean;
    issues: Issue[];
  };
  result?: {
    generation: number | null;
    status: string;
    message: string;
    health: { passed: number; total: number; failed: string[] };
  };
  trace: TraceStep[];
}

export interface AgentHealth {
  status: string;
  version: string;
  routing: string;
  local_only: boolean;
  knowledge_docs: number;
  providers: { id: string; model: string; local: boolean; circuit: string; recent_failures: number; last_error?: string }[];
}

// ------------------------------------------------------ user session --
// From flux-session (crates/flux-session), relayed by fluxd.

export interface UserSession {
  available: boolean;
  reason?: string;
  user?: string;
  uid?: number;
  home?: string;
  shell?: string;
  hostname?: string;
  kernel?: string;
  uptime_s?: number;
  load?: number[];
  cpus?: number;
  memory?: { total_kb: number | null; available_kb: number | null };
  disk?: { path: string; total: number; free: number } | null;
  wayland?: boolean;
}

export interface FileEntry {
  name: string;
  kind: "dir" | "file" | "link" | "other";
  symlink: boolean;
  size: number;
  modified: number | null;
  mode: string;
  uid: number;
  hidden: boolean;
}

export interface DirListing {
  path: string;
  parent: string | null;
  home: string;
  entries: FileEntry[];
  truncated: boolean;
}

export interface FileContent {
  path: string;
  size: number;
  modified: number | null;
  mime: string;
  text?: string;
  data_url?: string;
  binary?: boolean;
  truncated?: boolean;
}

export interface ProcessInfo {
  pid: number;
  ppid: number;
  name: string;
  state: string;
  uid: number;
  user: string;
  cpu_ticks: number;
  threads: number;
  start_ticks: number;
  rss: number;
  cmd: string;
  kernel: boolean;
}

export interface ProcessList {
  uid: number;
  total_ticks: number;
  ticks_per_second: number;
  cpus: number;
  processes: ProcessInfo[];
}

export interface DesktopApp {
  id: string;
  name: string;
  generic?: string | null;
  comment?: string | null;
  icon?: string | null;
  categories: string[];
  terminal: boolean;
}
