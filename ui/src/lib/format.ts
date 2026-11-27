import type { GenStatus, PlanStatus, Tier } from "./types";

export const TIER_LABEL = ["Safe", "Low", "Medium", "High", "Blocked"] as const;

export const TIER_BEHAVIOR = [
  "Applied automatically with a snapshot",
  "Applied automatically, undo offered",
  "Needs your confirmation",
  "Needs explicit approval and keeps the previous boot entry",
  "Never allowed",
] as const;

/** Tailwind text/background classes per tier. */
export function tierTone(t: Tier | number): { text: string; bg: string; dot: string } {
  switch (t) {
    case 0:
      return { text: "text-ok", bg: "bg-ok/15", dot: "bg-ok" };
    case 1:
      return { text: "text-accent", bg: "bg-accent/15", dot: "bg-accent" };
    case 2:
      return { text: "text-warn", bg: "bg-warn/15", dot: "bg-warn" };
    default:
      return { text: "text-bad", bg: "bg-bad/15", dot: "bg-bad" };
  }
}

export function ago(ts: string | number | Date, now = Date.now()): string {
  const t = new Date(ts).getTime();
  const s = Math.max(0, Math.round((now - t) / 1000));
  if (s < 10) return "just now";
  if (s < 60) return `${s}s ago`;
  const m = Math.round(s / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h}h ago`;
  return new Date(t).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

export function clockTime(d = new Date()): string {
  return d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
}

export function dateTime(ts: string): string {
  return new Date(ts).toLocaleString(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

export function shortHash(h: string, n = 10): string {
  const v = h.replace(/^sha256:/, "");
  return `${v.slice(0, n)}…`;
}

export function titleCase(s: string): string {
  return s.replace(/[-_]/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
}

export const DESKTOP_LABEL: Record<string, string> = {
  fluxos: "FluxOS shell",
  plasma: "KDE Plasma",
  gnome: "GNOME",
  hyprland: "Hyprland",
  cosmic: "COSMIC",
};

export function desktopLabel(env: string): string {
  return DESKTOP_LABEL[env] ?? titleCase(env);
}

/** 1536 -> "1.5 KB" (powers of 1024, like file managers). */
export function bytes(n: number | null | undefined): string {
  if (n === null || n === undefined || !Number.isFinite(n)) return "–";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${i === 0 ? v : v.toFixed(v < 10 ? 1 : 0)} ${units[i]}`;
}

/** Seconds -> "3 h 12 min". */
export function duration(s: number | null | undefined): string {
  if (!s && s !== 0) return "–";
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (d) return `${d} d ${h} h`;
  if (h) return `${h} h ${m} min`;
  return `${m} min`;
}

export function isHexColor(v: unknown): v is string {
  return typeof v === "string" && /^#[0-9a-f]{6}$/i.test(v);
}

export function valueText(v: unknown): string {
  if (v === null || v === undefined) return "unset";
  if (typeof v === "string") return v;
  if (typeof v === "boolean") return v ? "on" : "off";
  if (Array.isArray(v)) return v.length ? v.join(", ") : "none";
  return JSON.stringify(v);
}

export const PLAN_STATUS: Record<PlanStatus, { label: string; tone: string }> = {
  proposed: { label: "Waiting", tone: "text-accent" },
  approved: { label: "Approved", tone: "text-accent" },
  applied: { label: "Applied", tone: "text-ok" },
  staged: { label: "Staged", tone: "text-warn" },
  rejected: { label: "Rejected", tone: "text-muted" },
  rolled_back: { label: "Rolled back", tone: "text-bad" },
  failed: { label: "Failed", tone: "text-bad" },
  blocked: { label: "Refused", tone: "text-bad" },
  invalid: { label: "Invalid", tone: "text-bad" },
  no_op: { label: "No change", tone: "text-muted" },
};

export const GEN_STATUS: Record<GenStatus, { label: string; tone: string }> = {
  pending: { label: "Pending", tone: "text-muted" },
  active: { label: "Active", tone: "text-ok" },
  superseded: { label: "Kept", tone: "text-muted" },
  staged: { label: "Staged", tone: "text-warn" },
  failed: { label: "Failed", tone: "text-bad" },
  "rolled-back": { label: "Rolled back", tone: "text-bad" },
};

/** "Chrome on Linux" from the user agent, used to name paired screens. */
export function screenName(): string {
  const ua = navigator.userAgent;
  const browser = /Edg\//.test(ua)
    ? "Edge"
    : /Firefox\//.test(ua)
      ? "Firefox"
      : /Chrome\//.test(ua)
        ? "Chrome"
        : /Safari\//.test(ua)
          ? "Safari"
          : "Browser";
  const os = /Windows/.test(ua) ? "Windows" : /Mac OS X/.test(ua) ? "macOS" : /Android/.test(ua) ? "Android" : /Linux/.test(ua) ? "Linux" : "this device";
  return `${browser} on ${os}`;
}
