import type { LucideIcon } from "lucide-react";
import { LayoutGrid, Sparkles } from "lucide-react";
import { type CSSProperties, useMemo, useState } from "react";
import { useShallow } from "zustand/react/shallow";

import { APPS, DOCK_APPS, TINT_BG } from "../apps/meta";
import { category, desktopEntryFor, hueGradient, installedApps } from "../lib/apps";
import type { Spec } from "../lib/types";
import { useShell } from "../state/shell";
import { selectPending, useSystem } from "../state/system";
import { useUser } from "../state/user";
import { DOCK_PAD } from "./layout";

export interface DockItem {
  key: string;
  label: string;
  icon: LucideIcon;
  background: string;
  onClick: () => void;
  running?: boolean;
  badge?: number;
  separatorBefore?: boolean;
}

/** FluxOS apps, then installed GUI apps, then the app grid. */
export function useDockItems(spec: Spec, maxInstalled = 8): DockItem[] {
  const { wins, focused, overlay } = useShell(useShallow((s) => ({ wins: s.wins, focused: s.focused, overlay: s.overlay })));
  const pending = useSystem((s) => selectPending(s).length);
  const catalog = useSystem((s) => s.catalog);
  const entries = useUser((s) => s.apps);
  return useMemo(() => {
    const shell = useShell.getState();
    const items: DockItem[] = [
      {
        key: "assistant",
        label: "Ask Flux",
        icon: Sparkles,
        background: "linear-gradient(140deg, var(--flux-color-accent), var(--flux-color-secondary))",
        onClick: () => shell.toggle("assistant"),
        running: overlay === "assistant",
      },
    ];
    for (const id of DOCK_APPS) {
      const meta = APPS[id];
      const win = wins.find((w) => w.app === id);
      items.push({
        key: id,
        label: meta.title,
        icon: meta.icon,
        background: TINT_BG[meta.tint],
        running: Boolean(win),
        badge: id === "approvals" && pending > 0 ? pending : undefined,
        onClick: () => {
          const s = useShell.getState();
          if (win && !win.minimized && focused === win.id) s.minimize(win.id);
          else s.open(id);
        },
      });
    }
    const apps = installedApps(spec, catalog).filter((a) => a.gui).slice(0, maxInstalled);
    apps.forEach((a, i) => {
      const cat = category(a.category);
      // Started in the user's session when flux-session lists its desktop entry.
      const entry = desktopEntryFor(a, entries);
      items.push({
        key: `app:${a.id}`,
        label: a.name,
        icon: cat.icon,
        background: hueGradient(cat.hue),
        separatorBefore: i === 0,
        onClick: () => (entry ? void useUser.getState().launch(entry) : useShell.setState({ overlay: "launcher" })),
      });
    });
    items.push({
      key: "launcher",
      label: "All apps",
      icon: LayoutGrid,
      background: "color-mix(in oklab, var(--flux-color-text) 14%, transparent)",
      separatorBefore: true,
      onClick: () => shell.toggle("launcher"),
      running: overlay === "launcher",
    });
    return items;
  }, [wins, focused, overlay, pending, catalog, entries, spec, maxInstalled]);
}

export function DockIcon({
  item,
  size,
  scale = 1,
  vertical = false,
  edge = "bottom",
  tooltip = true,
}: {
  item: DockItem;
  size: number;
  scale?: number;
  vertical?: boolean;
  edge?: "bottom" | "left" | "right" | "top";
  tooltip?: boolean;
}) {
  const Icon = item.icon;
  const lift = (scale - 1) * size * 0.5;
  const transform =
    edge === "bottom" ? `translateY(${-lift}px) scale(${scale})` : edge === "top" ? `translateY(${lift}px) scale(${scale})` : edge === "left" ? `translateX(${lift}px) scale(${scale})` : `translateX(${-lift}px) scale(${scale})`;
  const origin = { bottom: "50% 100%", top: "50% 0%", left: "0% 50%", right: "100% 50%" }[edge];
  const tip: CSSProperties =
    edge === "bottom"
      ? { bottom: "calc(100% + 14px)", left: "50%", transform: "translateX(-50%)" }
      : edge === "top"
        ? { top: "calc(100% + 10px)", left: "50%", transform: "translateX(-50%)" }
        : edge === "left"
          ? { left: "calc(100% + 14px)", top: "50%", transform: "translateY(-50%)" }
          : { right: "calc(100% + 14px)", top: "50%", transform: "translateY(-50%)" };
  return (
    <button
      type="button"
      onClick={item.onClick}
      aria-label={item.label}
      className="group relative shrink-0 outline-none"
      style={{ width: size, height: size }}
    >
      <span
        className="absolute inset-0 grid place-items-center rounded-[28%] text-white shadow-[0_6px_16px_-6px_rgb(0_0_0/0.55),inset_0_1px_0_rgb(255_255_255/0.25)] transition-transform duration-150 group-focus-visible:ring-2 group-focus-visible:ring-accent"
        style={{ background: item.background, transform, transformOrigin: origin }}
      >
        <Icon style={{ width: size * 0.5, height: size * 0.5 }} strokeWidth={2.1} className="drop-shadow-[0_1px_1px_rgb(0_0_0/0.35)]" />
        {item.badge !== undefined && (
          <span className="absolute -top-1 -right-1 grid h-[18px] min-w-[18px] place-items-center rounded-full bg-bad px-1 text-[11px] font-bold text-white ring-2 ring-bg">
            {item.badge}
          </span>
        )}
      </span>
      {item.running && (
        <span
          className={`absolute rounded-full bg-fg/80 ${vertical ? "top-1/2 h-1.5 w-1 -translate-y-1/2" : "left-1/2 h-1 w-1.5 -translate-x-1/2"}`}
          style={edge === "bottom" ? { bottom: -6 } : edge === "top" ? { top: -5 } : edge === "left" ? { left: -6 } : { right: -6 }}
        />
      )}
      {tooltip && (
        <span
          className="pointer-events-none absolute z-50 rounded-md border hairline bg-raised px-2 py-1 text-xs font-medium whitespace-nowrap text-fg opacity-0 shadow-raised transition-opacity duration-100 group-hover:opacity-100 group-focus-visible:opacity-100"
          style={tip}
        >
          {item.label}
        </span>
      )}
    </button>
  );
}

function Separator({ vertical }: { vertical: boolean }) {
  return <span className={`shrink-0 bg-fg/15 ${vertical ? "my-0.5 h-px w-3/4" : "mx-0.5 h-3/5 w-px"}`} />;
}

/** The dock, laid out from the spec: position, size, autohide and floating. */
export function Dock({ spec }: { spec: Spec }) {
  const d = spec.desktop.dock;
  const vertical = d.position !== "bottom";
  const items = useDockItems(spec);
  const [hover, setHover] = useState<number | null>(null);
  const [revealed, setRevealed] = useState(false);
  const panel = spec.desktop.panel;
  const hidden = d.autohide && !revealed;
  const margin = d.floating ? 10 : 0;
  const bottomOffset = (panel.position === "bottom" ? panel.height_px : 0) + margin;
  const sideTop = panel.position === "top" ? panel.height_px : 0;

  const style: CSSProperties =
    d.position === "bottom"
      ? { left: "50%", bottom: bottomOffset, transform: `translateX(-50%) translateY(${hidden ? "130%" : "0"})` }
      : {
          top: `calc(50% + ${sideTop / 2}px)`,
          [d.position]: margin,
          transform: `translateY(-50%) translateX(${hidden ? (d.position === "left" ? "-130%" : "130%") : "0"})`,
        };

  const scale = (i: number) => {
    if (hover === null) return 1;
    const dist = Math.abs(i - hover);
    return dist === 0 ? 1.32 : dist === 1 ? 1.14 : dist === 2 ? 1.04 : 1;
  };

  return (
    <>
      {d.autohide && (
        <div
          aria-hidden="true"
          className="fixed z-30"
          style={d.position === "bottom" ? { left: 0, right: 0, bottom: bottomOffset - margin, height: 8 } : { top: 0, bottom: 0, [d.position]: 0, width: 8 }}
          onMouseEnter={() => setRevealed(true)}
        />
      )}
      <nav
        aria-label="Dock"
        onMouseLeave={() => {
          setHover(null);
          if (d.autohide) setRevealed(false);
        }}
        onFocus={() => setRevealed(true)}
        className={`glass chrome-reveal fixed z-30 flex items-center border hairline shadow-raised transition-transform duration-300 ${vertical ? "flex-col" : ""}`}
        style={{
          ...style,
          gap: Math.round(d.size_px * 0.16),
          padding: DOCK_PAD,
          borderRadius: d.floating ? `calc(var(--flux-radius-lg) * 1.35)` : d.position === "bottom" ? "var(--flux-radius-lg) var(--flux-radius-lg) 0 0" : 0,
        }}
      >
        {items.map((item, i) => (
          <div key={item.key} className={`flex items-center ${vertical ? "flex-col" : ""}`} style={{ gap: Math.round(d.size_px * 0.16) }} onMouseEnter={() => setHover(i)}>
            {item.separatorBefore && <Separator vertical={vertical} />}
            <DockIcon item={item} size={d.size_px} scale={scale(i)} vertical={vertical} edge={d.position} />
          </div>
        ))}
      </nav>
    </>
  );
}
