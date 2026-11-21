import type { LucideIcon } from "lucide-react";
import { Activity, Cpu, FileCode, GitCommitVertical, Inbox, Layers } from "lucide-react";

export type AppId = "approvals" | "timeline" | "system" | "profiles" | "activity" | "viewer";

export interface AppMeta {
  id: AppId;
  title: string;
  icon: LucideIcon;
  size: [number, number];
  singleton: boolean;
  /** Theme color the icon tile uses. */
  tint: "accent" | "secondary" | "ok" | "warn";
  blurb: string;
}

export const APPS: Record<AppId, AppMeta> = {
  approvals: { id: "approvals", title: "Approvals", icon: Inbox, size: [940, 620], singleton: true, tint: "warn", blurb: "Changes that need a person" },
  timeline: { id: "timeline", title: "Timeline", icon: GitCommitVertical, size: [780, 640], singleton: true, tint: "secondary", blurb: "Every generation, with undo" },
  system: { id: "system", title: "System", icon: Cpu, size: [960, 660], singleton: true, tint: "accent", blurb: "Machine, design tokens and generated config" },
  profiles: { id: "profiles", title: "Profiles", icon: Layers, size: [1000, 680], singleton: true, tint: "accent", blurb: "Industry setups to apply or share" },
  activity: { id: "activity", title: "Activity", icon: Activity, size: [880, 620], singleton: true, tint: "ok", blurb: "Tamper-evident audit log" },
  viewer: { id: "viewer", title: "Config", icon: FileCode, size: [860, 640], singleton: false, tint: "secondary", blurb: "Generated files" },
};

export const DOCK_APPS: AppId[] = ["approvals", "timeline", "system", "profiles", "activity"];

export const TINT_BG: Record<AppMeta["tint"], string> = {
  accent: "linear-gradient(145deg, var(--flux-color-accent), color-mix(in oklab, var(--flux-color-accent) 55%, var(--flux-color-background)))",
  secondary: "linear-gradient(145deg, var(--flux-color-secondary), color-mix(in oklab, var(--flux-color-secondary) 55%, var(--flux-color-background)))",
  ok: "linear-gradient(145deg, var(--flux-color-success), color-mix(in oklab, var(--flux-color-success) 55%, var(--flux-color-background)))",
  warn: "linear-gradient(145deg, var(--flux-color-warning), color-mix(in oklab, var(--flux-color-warning) 55%, var(--flux-color-background)))",
};
