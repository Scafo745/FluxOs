import type { LucideIcon } from "lucide-react";
import { Activity, Cpu, FileCode, FolderOpen, Gauge, GitCommitVertical, Inbox, Layers, ShieldCheck, SquareTerminal, WandSparkles } from "lucide-react";

export type AppId =
  | "terminal"
  | "files"
  | "tasks"
  | "security"
  | "synthesizer"
  | "approvals"
  | "timeline"
  | "system"
  | "profiles"
  | "activity"
  | "viewer";

export interface AppMeta {
  id: AppId;
  title: string;
  icon: LucideIcon;
  size: [number, number];
  singleton: boolean;
  /** Theme color the icon tile uses. */
  tint: "accent" | "secondary" | "ok" | "warn" | "ink";
  blurb: string;
}

export const APPS: Record<AppId, AppMeta> = {
  terminal: { id: "terminal", title: "Terminal", icon: SquareTerminal, size: [760, 460], singleton: false, tint: "ink", blurb: "Your shell on this machine" },
  files: { id: "files", title: "Files", icon: FolderOpen, size: [900, 580], singleton: false, tint: "secondary", blurb: "Your files and the live theme" },
  tasks: { id: "tasks", title: "Tasks", icon: Gauge, size: [880, 620], singleton: true, tint: "ok", blurb: "Processes, CPU and memory" },
  security: { id: "security", title: "Security", icon: ShieldCheck, size: [880, 660], singleton: true, tint: "ok", blurb: "Posture, refusals and lock-down" },
  synthesizer: { id: "synthesizer", title: "Synthesizer", icon: WandSparkles, size: [920, 660], singleton: true, tint: "accent", blurb: "Looks and prompts for the whole OS" },
  approvals: { id: "approvals", title: "Approvals", icon: Inbox, size: [940, 620], singleton: true, tint: "warn", blurb: "Changes that need a person" },
  timeline: { id: "timeline", title: "Timeline", icon: GitCommitVertical, size: [780, 640], singleton: true, tint: "secondary", blurb: "Every generation, with undo" },
  system: { id: "system", title: "System", icon: Cpu, size: [960, 660], singleton: true, tint: "accent", blurb: "Machine, design tokens and generated config" },
  profiles: { id: "profiles", title: "Profiles", icon: Layers, size: [1000, 680], singleton: true, tint: "accent", blurb: "Industry setups to apply or share" },
  activity: { id: "activity", title: "Activity", icon: Activity, size: [880, 620], singleton: true, tint: "ok", blurb: "Tamper-evident audit log" },
  viewer: { id: "viewer", title: "Config", icon: FileCode, size: [860, 640], singleton: false, tint: "secondary", blurb: "Generated files" },
};

/** In the dock, after Ask Flux. */
export const DOCK_APPS: AppId[] = ["terminal", "files", "synthesizer", "security", "approvals", "timeline", "system"];

/** Every FluxOS app, for the launcher. */
export const LAUNCHER_APPS: AppId[] = ["terminal", "files", "synthesizer", "security", "tasks", "approvals", "timeline", "system", "profiles", "activity"];

/** Shortcuts on the desktop, as in the MorphOS prototype. */
export const DESKTOP_APPS: AppId[] = ["terminal", "files", "security", "synthesizer"];

export const TINT_BG: Record<AppMeta["tint"], string> = {
  accent: "linear-gradient(145deg, var(--flux-color-accent), color-mix(in oklab, var(--flux-color-accent) 55%, var(--flux-color-background)))",
  secondary: "linear-gradient(145deg, var(--flux-color-secondary), color-mix(in oklab, var(--flux-color-secondary) 55%, var(--flux-color-background)))",
  ok: "linear-gradient(145deg, var(--flux-color-success), color-mix(in oklab, var(--flux-color-success) 55%, var(--flux-color-background)))",
  warn: "linear-gradient(145deg, var(--flux-color-warning), color-mix(in oklab, var(--flux-color-warning) 55%, var(--flux-color-background)))",
  ink: "linear-gradient(145deg, color-mix(in oklab, var(--flux-color-text) 30%, #1d1d24), #111116)",
};
