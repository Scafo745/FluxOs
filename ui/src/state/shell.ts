// Window manager and desktop chrome state. Geometry rules (tiling, gaps,
// snapping, where the dock and panel sit) come from the OS spec, so the web
// shell behaves like the desktop FluxOS generated.

import { create } from "zustand";

import { APPS, type AppId } from "../apps/meta";
import type { Change, Tier } from "../lib/types";

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Win {
  id: string;
  app: AppId;
  title: string;
  /** Floating geometry; maximized, snapped and tiled layouts are derived. */
  rect: Rect;
  z: number;
  minimized: boolean;
  maximized: boolean;
  snap: "left" | "right" | null;
  workspace: number;
  props: Record<string, unknown>;
}

export type Tone = "info" | "success" | "warn" | "danger" | "accent";

export interface Toast {
  id: string;
  tone: Tone;
  title: string;
  body?: string;
  action?: { label: string; run: () => void };
  ttl: number;
}

export interface Confirm {
  title: string;
  body?: string;
  tier: Tier;
  changes: Change[];
  confirmLabel: string;
  onConfirm: () => Promise<void> | void;
}

export type Overlay = "assistant" | "control" | "launcher";

interface ShellState {
  wins: Win[];
  focused: string | null;
  z: number;
  workspace: number;
  area: Rect;
  overlay: Overlay | null;
  toasts: Toast[];
  confirm: Confirm | null;
  snapPreview: Rect | null;
  setArea(area: Rect): void;
  open(app: AppId, props?: Record<string, unknown>, title?: string): string;
  close(id: string): void;
  focus(id: string): void;
  minimize(id: string): void;
  toggleMax(id: string): void;
  move(id: string, rect: Partial<Rect>, flags?: Partial<Pick<Win, "maximized" | "snap">>): void;
  setSnap(id: string, snap: Win["snap"] | "max"): void;
  setWorkspace(n: number): void;
  show(o: Overlay | null): void;
  toggle(o: Overlay): void;
  toast(t: Omit<Toast, "id" | "ttl"> & { ttl?: number }): void;
  dismiss(id: string): void;
  ask(c: Confirm): void;
  settle(): void;
  setSnapPreview(r: Rect | null): void;
}

let seq = 0;

function clampRect(r: Rect, area: Rect): Rect {
  const w = Math.min(r.w, area.w);
  const h = Math.min(r.h, area.h);
  return {
    w,
    h,
    x: Math.min(Math.max(r.x, area.x - w + 120), area.x + area.w - 120),
    y: Math.min(Math.max(r.y, area.y), area.y + area.h - 44),
  };
}

export const useShell = create<ShellState>()((set, get) => ({
  wins: [],
  focused: null,
  z: 1,
  workspace: 1,
  area: { x: 0, y: 28, w: 1280, h: 700 },
  overlay: null,
  toasts: [],
  confirm: null,
  snapPreview: null,

  setArea: (area) =>
    set((s) => ({ area, wins: s.wins.map((w) => ({ ...w, rect: clampRect(w.rect, area) })) })),

  open: (app, props = {}, title) => {
    const s = get();
    const meta = APPS[app];
    const existing = meta.singleton ? s.wins.find((w) => w.app === app) : undefined;
    const z = s.z + 1;
    if (existing) {
      set({
        z,
        focused: existing.id,
        overlay: null,
        wins: s.wins.map((w) =>
          w.id === existing.id ? { ...w, z, minimized: false, workspace: s.workspace, props: { ...w.props, ...props } } : w,
        ),
      });
      return existing.id;
    }
    const area = s.area;
    const onWs = s.wins.filter((w) => w.workspace === s.workspace && !w.minimized).length;
    const w = Math.min(meta.size[0], area.w - 32);
    const h = Math.min(meta.size[1], area.h - 32);
    const offset = (onWs % 6) * 28;
    const rect = {
      w,
      h,
      x: Math.round(area.x + (area.w - w) / 2 - 70 + offset),
      y: Math.round(area.y + Math.max(16, (area.h - h) / 2 - 30) + offset),
    };
    const id = `w${++seq}`;
    set({
      z,
      focused: id,
      overlay: null,
      wins: [
        ...s.wins,
        { id, app, title: title ?? meta.title, rect: clampRect(rect, area), z, minimized: false, maximized: false, snap: null, workspace: s.workspace, props },
      ],
    });
    return id;
  },

  close: (id) =>
    set((s) => {
      const wins = s.wins.filter((w) => w.id !== id);
      const next = wins.filter((w) => w.workspace === s.workspace && !w.minimized).sort((a, b) => b.z - a.z)[0];
      return { wins, focused: s.focused === id ? (next?.id ?? null) : s.focused };
    }),

  focus: (id) =>
    set((s) => {
      if (s.focused === id && s.wins.find((w) => w.id === id)?.z === s.z) return s;
      const z = s.z + 1;
      return { z, focused: id, wins: s.wins.map((w) => (w.id === id ? { ...w, z, minimized: false } : w)) };
    }),

  minimize: (id) =>
    set((s) => ({ wins: s.wins.map((w) => (w.id === id ? { ...w, minimized: true } : w)), focused: s.focused === id ? null : s.focused })),

  toggleMax: (id) => set((s) => ({ wins: s.wins.map((w) => (w.id === id ? { ...w, maximized: !w.maximized, snap: null } : w)) })),

  move: (id, rect, flags) =>
    set((s) => ({ wins: s.wins.map((w) => (w.id === id ? { ...w, ...flags, rect: { ...w.rect, ...rect } } : w)) })),

  setSnap: (id, snap) =>
    set((s) => ({
      wins: s.wins.map((w) =>
        w.id === id ? (snap === "max" ? { ...w, maximized: true, snap: null } : { ...w, snap, maximized: false }) : w,
      ),
    })),

  setWorkspace: (n) => set({ workspace: n, focused: null }),

  show: (overlay) => set({ overlay }),
  toggle: (o) => set((s) => ({ overlay: s.overlay === o ? null : o })),

  toast: (t) => {
    const id = `t${++seq}`;
    const ttl = t.ttl ?? (t.action ? 9000 : 6000);
    set((s) => ({ toasts: [...s.toasts.slice(-4), { ...t, id, ttl }] }));
    if (ttl > 0) setTimeout(() => get().dismiss(id), ttl);
  },
  dismiss: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),

  ask: (confirm) => set({ confirm }),
  settle: () => set({ confirm: null }),
  setSnapPreview: (snapPreview) => set({ snapPreview }),
}));
