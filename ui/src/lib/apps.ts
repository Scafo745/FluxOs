// Presentation metadata for installed software. Icons are original glyphs
// per category (no third-party logos), tinted by a hue per category.

import type { LucideIcon } from "lucide-react";
import {
  AppWindow,
  Code,
  FileText,
  Film,
  Gamepad2,
  Gauge,
  Globe,
  GraduationCap,
  KeyRound,
  MessageSquare,
  Monitor,
  Network,
  Paintbrush,
  Video,
} from "lucide-react";

import type { Catalog, Spec } from "./types";

export const CATEGORY: Record<string, { label: string; icon: LucideIcon; hue: number }> = {
  gaming: { label: "Games", icon: Gamepad2, hue: 188 },
  social: { label: "Social", icon: MessageSquare, hue: 245 },
  media: { label: "Media", icon: Film, hue: 330 },
  web: { label: "Web", icon: Globe, hue: 212 },
  office: { label: "Office", icon: FileText, hue: 150 },
  creative: { label: "Creative", icon: Paintbrush, hue: 18 },
  security: { label: "Security", icon: KeyRound, hue: 44 },
  meetings: { label: "Meetings", icon: Video, hue: 170 },
  remote: { label: "Remote access", icon: Monitor, hue: 265 },
  dev: { label: "Development", icon: Code, hue: 285 },
  education: { label: "Education", icon: GraduationCap, hue: 32 },
  network: { label: "Network", icon: Network, hue: 158 },
  system: { label: "System", icon: Gauge, hue: 222 },
  desktop: { label: "Desktop", icon: AppWindow, hue: 300 },
};

export function category(id: string) {
  return CATEGORY[id] ?? { label: "Apps", icon: AppWindow, hue: 220 };
}

/** Command-line tools: listed in the app grid, kept out of the dock. */
const CLI = new Set([
  "git", "python3", "nodejs", "neovim", "jq", "ripgrep", "tcpdump", "nmap", "mtr", "iperf3", "net-snmp",
  "netconf-client", "sngrep", "sipp", "baresip", "minicom", "picocom", "tmux", "ansible", "ipcalc", "dnsutils",
  "btop", "htop", "iftop", "nethogs", "termshark", "mangohud", "winetricks", "wine", "gamescope",
]);

export interface InstalledApp {
  id: string;
  name: string;
  description: string;
  category: string;
  source: "system" | "flatpak";
  gui: boolean;
}

export function installedApps(spec: Spec, catalog: Catalog | null): InstalledApp[] {
  const byId = new Map((catalog?.packages ?? []).map((p) => [p.id, p]));
  const out: InstalledApp[] = [];
  const add = (id: string, source: InstalledApp["source"]) => {
    if (out.some((a) => a.id === id)) return;
    const p = byId.get(id);
    out.push({ id, name: p?.name ?? id, description: p?.description ?? "", category: p?.category ?? "system", source, gui: !CLI.has(id) });
  };
  spec.packages.flatpak.forEach((id) => add(id, "flatpak"));
  spec.packages.system.forEach((id) => add(id, "system"));
  return out;
}

export function hueGradient(hue: number): string {
  return `linear-gradient(145deg, hsl(${hue} 72% 60%), hsl(${(hue + 28) % 360} 70% 42%))`;
}
