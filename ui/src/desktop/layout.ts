import { useEffect, useState } from "react";

import type { Spec } from "../lib/types";
import type { Rect, Win } from "../state/shell";

export const DOCK_PAD = 8;

/** Space the dock takes from the screen edge. */
export function dockThickness(spec: Spec): number {
  const d = spec.desktop.dock;
  return d.size_px + DOCK_PAD * 2 + (d.floating ? 12 : 0);
}

/** Screen area left for windows after the panel and a non-hiding dock. */
export function workArea(spec: Spec, vw: number, vh: number): Rect {
  const d = spec.desktop;
  const top = d.panel.position === "top" ? d.panel.height_px : 0;
  let bottom = d.panel.position === "bottom" ? d.panel.height_px : 0;
  let left = 0;
  let right = 0;
  if (d.dock.enabled && !d.dock.autohide) {
    const t = dockThickness(spec);
    if (d.dock.position === "bottom") bottom += t;
    else if (d.dock.position === "left") left = t;
    else right = t;
  }
  return { x: left, y: top, w: Math.max(320, vw - left - right), h: Math.max(240, vh - top - bottom) };
}

/**
 * Window geometry. With `windows.tiling` the shell tiles like the generated
 * Hyprland/KWin setup (master and stack, spec gaps); otherwise windows float
 * and may be snapped to halves or maximized.
 */
export function layoutWindows(wins: Win[], area: Rect, spec: Spec): Map<string, Rect> {
  const map = new Map<string, Rect>();
  const gap = spec.desktop.windows.gaps_px;
  if (spec.desktop.windows.tiling) {
    const inner = { x: area.x + gap, y: area.y + gap, w: area.w - gap * 2, h: area.h - gap * 2 };
    if (wins.some((w) => w.maximized)) {
      wins.forEach((w) => map.set(w.id, inner));
      return map;
    }
    const [master, ...stack] = wins;
    if (!master) return map;
    if (stack.length === 0) {
      map.set(master.id, inner);
      return map;
    }
    const masterW = Math.round((inner.w - gap) * 0.56);
    map.set(master.id, { x: inner.x, y: inner.y, w: masterW, h: inner.h });
    const sx = inner.x + masterW + gap;
    const sw = inner.w - masterW - gap;
    const each = (inner.h - gap * (stack.length - 1)) / stack.length;
    stack.forEach((w, i) => map.set(w.id, { x: sx, y: Math.round(inner.y + i * (each + gap)), w: sw, h: Math.round(each) }));
    return map;
  }
  for (const w of wins) {
    const half = Math.round(area.w / 2);
    if (w.maximized) map.set(w.id, area);
    else if (w.snap === "left") map.set(w.id, { x: area.x, y: area.y, w: half, h: area.h });
    else if (w.snap === "right") map.set(w.id, { x: area.x + half, y: area.y, w: area.w - half, h: area.h });
    else map.set(w.id, w.rect);
  }
  return map;
}

export function snapTarget(x: number, y: number, area: Rect): "left" | "right" | "max" | null {
  if (x <= area.x + 6) return "left";
  if (x >= area.x + area.w - 6) return "right";
  if (y <= area.y + 4) return "max";
  return null;
}

export function snapRect(snap: "left" | "right" | "max", area: Rect): Rect {
  const half = Math.round(area.w / 2);
  if (snap === "max") return area;
  return snap === "left" ? { x: area.x, y: area.y, w: half, h: area.h } : { x: area.x + half, y: area.y, w: area.w - half, h: area.h };
}

export function useViewport(): { w: number; h: number } {
  const [vp, setVp] = useState({ w: window.innerWidth, h: window.innerHeight });
  useEffect(() => {
    const on = () => setVp({ w: window.innerWidth, h: window.innerHeight });
    window.addEventListener("resize", on);
    return () => window.removeEventListener("resize", on);
  }, []);
  return vp;
}
