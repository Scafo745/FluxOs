import { Accessibility, Contrast, LogOut, Moon, Power, Shuffle, Sun, Type, Wind } from "lucide-react";
import { useState } from "react";

import { Spinner, TierBadge } from "../components/ui";
import { api } from "../lib/api";
import type { RawOp, Spec, StylePreset } from "../lib/types";
import { quick, restart } from "../state/actions";
import { useSession } from "../state/session";
import { useShell } from "../state/shell";
import { useSystem } from "../state/system";

const ACCENTS: [string, string][] = [
  ["#7c5cff", "Violet"],
  ["#3b82f6", "Blue"],
  ["#06b6d4", "Cyan"],
  ["#14b8a6", "Teal"],
  ["#22c55e", "Green"],
  ["#eab308", "Gold"],
  ["#f97316", "Ember"],
  ["#ef4444", "Red"],
  ["#ec4899", "Pink"],
  ["#a855f7", "Orchid"],
];
const WALLPAPERS = ["aurora", "gradient", "mesh", "grid", "solid"] as const;
const MOTION = ["snappy", "smooth", "calm", "minimal", "none"] as const;
const STYLES: [StylePreset, string][] = [
  ["glass-dock", "Glass dock"],
  ["snap-layouts", "Snap layouts"],
  ["tiling", "Tiling"],
  ["classic-panel", "Classic panel"],
  ["ops-center", "Ops center"],
  ["focus", "Focus"],
];

function Chip({ active, busy, onClick, children, title }: { active?: boolean; busy?: boolean; onClick: () => void; children: React.ReactNode; title?: string }) {
  return (
    <button
      type="button"
      title={title}
      onClick={onClick}
      disabled={busy}
      aria-pressed={active}
      className={`flex h-8 items-center justify-center gap-1.5 rounded-md border px-2.5 text-xs font-medium capitalize transition-colors ${
        active ? "border-accent bg-accent/15 text-fg" : "hairline bg-fg/[0.04] text-fg/85 hover:border-accent/50"
      }`}
    >
      {busy && <Spinner className="size-3" />}
      {children}
    </button>
  );
}

function Toggle({ label, icon: Icon, on, busy, onClick }: { label: string; icon: typeof Sun; on: boolean; busy?: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      onClick={onClick}
      disabled={busy}
      className={`flex flex-col items-start gap-2 rounded-lg border p-3 text-left text-xs font-medium transition-colors ${on ? "border-accent bg-accent text-on-accent" : "hairline bg-fg/[0.05] hover:border-accent/50"}`}
    >
      {busy ? <Spinner className="size-4" /> : <Icon className="size-4" />}
      {label}
    </button>
  );
}

export function ControlCenter({ spec }: { spec: Spec }) {
  const open = useShell((s) => s.overlay === "control");
  const system = useSystem((s) => s.system);
  const session = useSession((s) => s.session);
  const [busy, setBusy] = useState<string | null>(null);
  if (!open) return null;

  const t = spec.design.tokens;
  const a = spec.accessibility;
  const run = async (key: string, label: string, ops: RawOp[]) => {
    setBusy(key);
    await quick(label, ops);
    setBusy(null);
  };
  const top = spec.desktop.panel.position === "top";
  const offset = spec.desktop.panel.height_px + 8;

  return (
    <div className="fixed inset-0 z-[55]" onMouseDown={(e) => e.target === e.currentTarget && useShell.getState().show(null)}>
      <aside
        aria-label="Quick settings"
        className="glass absolute right-2 w-[360px] max-w-[calc(100vw-16px)] overflow-y-auto rounded-xl border hairline p-4 shadow-raised win-enter"
        style={top ? { top: offset, maxHeight: `calc(100vh - ${offset + 8}px)` } : { bottom: offset, maxHeight: `calc(100vh - ${offset + 8}px)` }}
      >
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <p className="text-xs tracking-[0.14em] text-muted uppercase">Theme</p>
            <p className="truncate font-display text-lg font-bold">{spec.design.theme_name}</p>
          </div>
          <span className="pt-1">
            <TierBadge tier={0} />
          </span>
        </div>
        <p className="mt-1 text-xs text-muted">Each setting is a tier-0 plan: policy-checked, snapshotted and undoable.</p>

        <div className="mt-4 grid grid-cols-2 gap-2">
          <Toggle
            label="Dark"
            icon={Moon}
            on={t.scheme === "dark"}
            busy={busy === "dark"}
            onClick={() => t.scheme !== "dark" && void run("dark", "Quick setting: dark mode", [{ tool: "design.set_palette", params: { scheme: "dark" } }])}
          />
          <Toggle
            label="Light"
            icon={Sun}
            on={t.scheme === "light"}
            busy={busy === "light"}
            onClick={() => t.scheme !== "light" && void run("light", "Quick setting: light mode", [{ tool: "design.set_palette", params: { scheme: "light" } }])}
          />
        </div>

        <p className="mt-4 mb-2 text-xs font-semibold tracking-[0.12em] text-muted uppercase">Accent</p>
        <div className="flex flex-wrap gap-2">
          {ACCENTS.map(([hex, name]) => (
            <button
              key={hex}
              type="button"
              title={name}
              aria-label={`${name} accent`}
              disabled={busy !== null}
              onClick={() =>
                void run(`accent${hex}`, `Quick setting: ${name.toLowerCase()} accent`, [
                  { tool: "design.generate_palette", params: { accent: hex, scheme: t.scheme, theme_name: `Flux ${name}` } },
                ])
              }
              className="grid size-7 place-items-center rounded-full ring-offset-2 ring-offset-bg transition-transform hover:scale-110 focus-visible:ring-2 focus-visible:ring-fg"
              style={{ background: hex }}
            >
              {busy === `accent${hex}` && <Spinner className="size-3.5 text-white" />}
            </button>
          ))}
        </div>

        <p className="mt-4 mb-2 text-xs font-semibold tracking-[0.12em] text-muted uppercase">Wallpaper</p>
        <div className="grid grid-cols-3 gap-1.5">
          {WALLPAPERS.map((w) => (
            <Chip
              key={w}
              active={spec.desktop.wallpaper.style === w}
              busy={busy === `wp${w}`}
              onClick={() => void run(`wp${w}`, `Quick setting: ${w} wallpaper`, [{ tool: "desktop.set_wallpaper", params: { style: w, seed: Math.floor(Math.random() * 9000) + 100 } }])}
            >
              {w}
            </Chip>
          ))}
          <Chip
            busy={busy === "shuffle"}
            title="New variation of the current style"
            onClick={() =>
              void run("shuffle", "Quick setting: new wallpaper", [
                { tool: "desktop.set_wallpaper", params: { style: spec.desktop.wallpaper.style, seed: Math.floor(Math.random() * 9000) + 100 } },
              ])
            }
          >
            <Shuffle className="size-3.5" /> Shuffle
          </Chip>
        </div>

        <p className="mt-4 mb-2 text-xs font-semibold tracking-[0.12em] text-muted uppercase">Style</p>
        <div className="grid grid-cols-3 gap-1.5">
          {STYLES.map(([id, label]) => (
            <Chip
              key={id}
              active={spec.desktop.style === id}
              busy={busy === `style${id}`}
              onClick={() => void run(`style${id}`, `Quick setting: ${label} style`, [{ tool: "desktop.apply_style", params: { style: id } }])}
            >
              {label}
            </Chip>
          ))}
        </div>

        <p className="mt-4 mb-2 text-xs font-semibold tracking-[0.12em] text-muted uppercase">Motion</p>
        <div className="grid grid-cols-5 gap-1.5">
          {MOTION.map((m) => (
            <Chip key={m} busy={busy === `motion${m}`} onClick={() => void run(`motion${m}`, `Quick setting: ${m} motion`, [{ tool: "design.set_motion", params: { preset: m } }])}>
              {m}
            </Chip>
          ))}
        </div>

        <p className="mt-4 mb-2 text-xs font-semibold tracking-[0.12em] text-muted uppercase">Accessibility</p>
        <div className="grid grid-cols-3 gap-2">
          <Toggle
            label="High contrast"
            icon={Contrast}
            on={a.high_contrast}
            busy={busy === "hc"}
            onClick={() => void run("hc", "Quick setting: high contrast", [{ tool: "accessibility.set", params: { high_contrast: !a.high_contrast } }])}
          />
          <Toggle
            label="Large text"
            icon={Type}
            on={a.large_text}
            busy={busy === "lt"}
            onClick={() =>
              void run("lt", "Quick setting: large text", [{ tool: "accessibility.set", params: { large_text: !a.large_text, text_scale: a.large_text ? 1 : 1.25 } }])
            }
          />
          <Toggle
            label="Reduce motion"
            icon={Wind}
            on={a.reduce_motion}
            busy={busy === "rm"}
            onClick={() => void run("rm", "Quick setting: reduce motion", [{ tool: "accessibility.set", params: { reduce_motion: !a.reduce_motion } }])}
          />
        </div>
        <button
          type="button"
          onClick={() => void run("sr", "Quick setting: screen reader", [{ tool: "accessibility.set", params: { screen_reader: !a.screen_reader } }])}
          className="mt-2 flex w-full items-center justify-between rounded-lg border hairline bg-fg/[0.04] px-3 py-2 text-xs font-medium hover:border-accent/50"
        >
          <span className="flex items-center gap-2">
            <Accessibility className="size-4" /> Screen reader (Orca)
          </span>
          <span className={a.screen_reader ? "text-ok" : "text-muted"}>{busy === "sr" ? <Spinner className="size-3.5" /> : a.screen_reader ? "On" : "Off"}</span>
        </button>

        <div className="mt-4 flex items-center gap-2 border-t hairline pt-3 text-xs text-muted">
          <span className="min-w-0 flex-1 truncate" title={session?.principal.id}>
            {session?.name} · {session?.principal.role === "admin" ? "owner" : session?.principal.role}
          </span>
          {system?.status === "staged" && (
            <button type="button" onClick={() => void restart()} className="flex items-center gap-1 rounded-md px-2 py-1 font-semibold text-warn hover:bg-warn/10">
              <Power className="size-3.5" /> Restart
            </button>
          )}
          <button
            type="button"
            onClick={async () => {
              try {
                await api.logout();
              } finally {
                useSession.getState().clear();
              }
            }}
            className="flex items-center gap-1 rounded-md px-2 py-1 hover:bg-fg/10 hover:text-fg"
          >
            <LogOut className="size-3.5" /> Sign out
          </button>
        </div>
      </aside>
    </div>
  );
}
