import { Maximize2, Minimize2, Minus, X } from "lucide-react";
import { type PointerEvent as ReactPointerEvent, useMemo, useState } from "react";
import { useShallow } from "zustand/react/shallow";

import { APPS, TINT_BG } from "../apps/meta";
import { APP_COMPONENTS } from "../apps/registry";
import type { Spec } from "../lib/types";
import { type Rect, type Win, useShell } from "../state/shell";
import { layoutWindows, snapRect, snapTarget } from "./layout";

export function AppGlyph({ app, size = 18 }: { app: Win["app"]; size?: number }) {
  const meta = APPS[app];
  const Icon = meta.icon;
  return (
    <span className="grid shrink-0 place-items-center rounded-[28%] text-white" style={{ width: size, height: size, background: TINT_BG[meta.tint] }}>
      <Icon style={{ width: size * 0.6, height: size * 0.6 }} strokeWidth={2.3} />
    </span>
  );
}

function WinButton({ label, icon: Icon, onClick, danger }: { label: string; icon: typeof X; onClick: () => void; danger?: boolean }) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      onPointerDown={(e) => e.stopPropagation()}
      className={`grid size-7 place-items-center rounded-md text-fg/70 transition-colors ${danger ? "hover:bg-bad hover:text-white" : "hover:bg-fg/10 hover:text-fg"}`}
    >
      <Icon className="size-4" />
    </button>
  );
}

function WindowFrame({ win, rect, focused, spec }: { win: Win; rect: Rect; focused: boolean; spec: Spec }) {
  const [dragging, setDragging] = useState(false);
  const tiling = spec.desktop.windows.tiling;
  const snapping = spec.desktop.windows.snapping;
  const radius = spec.desktop.windows.corner_radius_px;
  const Body = APP_COMPONENTS[win.app];

  const startDrag = (e: ReactPointerEvent) => {
    if (e.button !== 0 || tiling) return;
    e.preventDefault();
    const shell = useShell.getState();
    shell.focus(win.id);
    let base = rect;
    if (win.maximized || win.snap) {
      // Pull a maximized or snapped window back to its floating size under the pointer.
      const ratio = (e.clientX - rect.x) / rect.w;
      base = { ...win.rect, x: e.clientX - win.rect.w * ratio, y: rect.y };
      shell.move(win.id, base, { maximized: false, snap: null });
    }
    const sx = e.clientX;
    const sy = e.clientY;
    let target: ReturnType<typeof snapTarget> = null;
    setDragging(true);
    const onMove = (ev: PointerEvent) => {
      const area = useShell.getState().area;
      const x = base.x + ev.clientX - sx;
      const y = Math.min(Math.max(area.y, base.y + ev.clientY - sy), area.y + area.h - 44);
      useShell.getState().move(win.id, { x, y });
      if (snapping) {
        target = snapTarget(ev.clientX, ev.clientY, area);
        useShell.getState().setSnapPreview(target ? snapRect(target, area) : null);
      }
    };
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      setDragging(false);
      const s = useShell.getState();
      s.setSnapPreview(null);
      if (target) s.setSnap(win.id, target);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp, { once: true });
  };

  const startResize = (e: ReactPointerEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const sx = e.clientX;
    const sy = e.clientY;
    const start = rect;
    setDragging(true);
    const onMove = (ev: PointerEvent) =>
      useShell.getState().move(win.id, { w: Math.max(380, start.w + ev.clientX - sx), h: Math.max(260, start.h + ev.clientY - sy) });
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      setDragging(false);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp, { once: true });
  };

  const floating = !tiling && !win.maximized && !win.snap;
  return (
    <section
      role="dialog"
      aria-label={win.title}
      onPointerDown={() => useShell.getState().focus(win.id)}
      className={`win-enter pointer-events-auto absolute flex flex-col overflow-hidden border bg-surface ${
        focused ? (tiling ? "border-accent" : "border-line shadow-[0_24px_60px_-20px_rgb(0_0_0/0.6)]") : "border-line/70 shadow-[0_10px_30px_-18px_rgb(0_0_0/0.5)]"
      } ${dragging ? "" : "transition-[left,top,width,height]"}`}
      style={{ left: rect.x, top: rect.y, width: rect.w, height: rect.h, zIndex: 10 + win.z, borderRadius: win.maximized && !tiling ? 0 : radius }}
    >
      <header
        onPointerDown={startDrag}
        onDoubleClick={() => !tiling && useShell.getState().toggleMax(win.id)}
        className={`flex h-10 shrink-0 items-center gap-2.5 border-b hairline px-3 select-none ${focused ? "bg-raised" : "bg-raised/60"}`}
      >
        <AppGlyph app={win.app} />
        <h2 className={`truncate text-sm font-semibold ${focused ? "" : "text-muted"}`}>{win.title}</h2>
        <div className="flex-1" />
        <WinButton label="Minimize" icon={Minus} onClick={() => useShell.getState().minimize(win.id)} />
        {!tiling && (
          <WinButton label={win.maximized ? "Restore" : "Maximize"} icon={win.maximized ? Minimize2 : Maximize2} onClick={() => useShell.getState().toggleMax(win.id)} />
        )}
        <WinButton label="Close" icon={X} onClick={() => useShell.getState().close(win.id)} danger />
      </header>
      <div className="relative min-h-0 flex-1 overflow-auto select-text">
        <Body win={win} />
      </div>
      {floating && (
        <div
          aria-hidden="true"
          onPointerDown={startResize}
          className="absolute right-0 bottom-0 size-4 cursor-nwse-resize"
          style={{ background: "linear-gradient(135deg, transparent 50%, color-mix(in oklab, var(--flux-color-text) 25%, transparent) 50%)" }}
        />
      )}
    </section>
  );
}

export function WindowLayer({ spec }: { spec: Spec }) {
  const { wins, workspace, area, focused } = useShell(
    useShallow((s) => ({ wins: s.wins, workspace: s.workspace, area: s.area, focused: s.focused })),
  );
  const visible = useMemo(() => wins.filter((w) => w.workspace === workspace && !w.minimized), [wins, workspace]);
  const rects = useMemo(() => layoutWindows(visible, area, spec), [visible, area, spec]);
  return (
    <div className="pointer-events-none absolute inset-0">
      {visible.map((w) => {
        const r = rects.get(w.id);
        return r ? <WindowFrame key={w.id} win={w} rect={r} focused={focused === w.id} spec={spec} /> : null;
      })}
    </div>
  );
}

export function SnapPreview() {
  const r = useShell((s) => s.snapPreview);
  if (!r) return null;
  return (
    <div
      className="pointer-events-none fixed z-[5] rounded-xl border-2 border-accent/70 bg-accent/15 transition-all duration-150"
      style={{ left: r.x + 6, top: r.y + 6, width: r.w - 12, height: r.h - 12 }}
    />
  );
}
