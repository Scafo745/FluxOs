// Everything the UI knows about the machine, kept current by fluxd's event
// stream. Previews swap in a candidate spec and theme without applying them.

import { create } from "zustand";

import { api, fluxdUrl } from "../lib/api";
import type {
  AgentHealth,
  BootStage,
  Catalog,
  FluxEvent,
  GenerationMeta,
  PlanRecord,
  ProfileSummary,
  RawOp,
  Spec,
  SystemView,
  Tier,
  WhoAmI,
} from "../lib/types";
import { clientId, useSession } from "./session";
import { useShell } from "./shell";

export interface Preview {
  kind: "plan" | "generation";
  /** Plan id or generation number. */
  id: string;
  label: string;
  spec: Spec;
  css: string;
  wallpaperUrl: string;
  tier?: Tier;
}

export interface Restart {
  kind: "reboot" | "core";
  stages: BootStage[];
  ready: boolean;
  generation?: number;
  durationMs?: number;
}

interface SystemState {
  conn: "connecting" | "online" | "offline";
  bootId: string | null;
  system: SystemView | null;
  whoami: WhoAmI | null;
  themeCss: string | null;
  themeKey: number;
  plans: PlanRecord[];
  generations: GenerationMeta[];
  catalog: Catalog | null;
  profiles: ProfileSummary[];
  events: FluxEvent[];
  applying: { generation: number; planId: string } | null;
  preview: Preview | null;
  restart: Restart | null;
  agent: AgentHealth | "offline" | null;

  loadPublicTheme(): Promise<void>;
  loadAll(): Promise<void>;
  refreshSystem(): Promise<void>;
  refreshTheme(): Promise<void>;
  refreshPlans(): Promise<void>;
  refreshGenerations(): Promise<void>;
  loadProfiles(): Promise<void>;
  setConn(c: SystemState["conn"]): void;
  setAgent(a: SystemState["agent"]): void;
  onEvent(ev: FluxEvent): void;
  previewPlan(id: string, label: string, tier?: Tier): Promise<void>;
  previewGeneration(n: number): Promise<void>;
  endPreview(): void;
  endRestart(): void;
  /** A plan this UI starts directly (quick settings, profiles, app removal). */
  proposeUi(prompt: string, operations: RawOp[], autoApply: boolean): Promise<PlanRecord>;
}

export const selectSpec = (s: SystemState): Spec | null => s.preview?.spec ?? s.system?.spec ?? null;

/** Plans waiting for a person. */
export const selectPending = (s: SystemState): PlanRecord[] =>
  s.plans.filter((p) => (p.status === "proposed" || p.status === "approved") && p.decision !== "auto_apply" && p.decision !== "blocked");

let plansTimer: ReturnType<typeof setTimeout> | null = null;
let previewUrl: string | null = null;

function firstLine(s: unknown): string {
  return typeof s === "string" ? (s.split("\n")[0] ?? "") : "";
}

function svgUrl(svg: string): string {
  if (previewUrl) URL.revokeObjectURL(previewUrl);
  previewUrl = URL.createObjectURL(new Blob([svg], { type: "image/svg+xml" }));
  return previewUrl;
}

export const useSystem = create<SystemState>()((set, get) => ({
  conn: "connecting",
  bootId: null,
  system: null,
  whoami: null,
  themeCss: null,
  themeKey: 0,
  plans: [],
  generations: [],
  catalog: null,
  profiles: [],
  events: [],
  applying: null,
  preview: null,
  restart: null,
  agent: null,

  loadPublicTheme: async () => {
    const css = await api.themeCss();
    set({ themeCss: css });
  },

  loadAll: async () => {
    const s = get();
    await Promise.allSettled([
      s.refreshSystem(),
      s.refreshPlans(),
      s.refreshGenerations(),
      api.whoami().then((whoami) => set({ whoami })),
      s.catalog ? Promise.resolve() : api.catalog().then((catalog) => set({ catalog })),
      s.loadProfiles(),
    ]);
    await get().refreshTheme();
  },

  refreshSystem: async () => {
    const system = await api.system();
    set({ system });
  },

  refreshTheme: async () => {
    const css = await api.themeCss();
    set((s) => ({ themeCss: css, themeKey: s.system?.generation ?? s.themeKey + 1 }));
  },

  refreshPlans: async () => {
    const plans = await api.plans(80);
    set({ plans });
  },

  refreshGenerations: async () => {
    const r = await api.generations();
    set({ generations: r.generations });
  },

  loadProfiles: async () => {
    const profiles = await api.profiles();
    set({ profiles });
  },

  setConn: (conn) => set({ conn }),
  setAgent: (agent) => set({ agent }),

  onEvent: (ev) => {
    set((s) => ({ events: [...s.events.slice(-199), ev] }));
    const d = ev.data ?? {};
    const mine = d.origin?.session_id === clientId || d.session_id === clientId;
    const shell = useShell.getState();
    const plansSoon = () => {
      if (plansTimer) clearTimeout(plansTimer);
      plansTimer = setTimeout(() => void get().refreshPlans().catch(() => {}), 150);
    };

    switch (ev.kind) {
      case "boot.begin": {
        if (d.reboot) {
          set((s) => ({ restart: s.restart?.kind === "reboot" ? s.restart : { kind: "reboot", stages: [], ready: false } }));
        } else if (get().bootId && d.boot_id && d.boot_id !== get().bootId) {
          // fluxd restarted underneath us: show its new boot.
          set({ restart: { kind: "core", stages: [], ready: false } });
        }
        break;
      }
      case "boot.stage":
        set((s) => (s.restart && !s.restart.ready ? { restart: { ...s.restart, stages: [...s.restart.stages, d as BootStage] } } : s));
        break;
      case "boot.ready":
        if (d.boot_id) set({ bootId: d.boot_id });
        set((s) => (s.restart ? { restart: { ...s.restart, ready: true, generation: d.generation, durationMs: d.duration_ms } } : s));
        void get().loadAll().catch(() => {});
        break;
      case "system.reboot":
        set((s) => ({ restart: s.restart ?? { kind: "reboot", stages: [], ready: false, generation: d.generation } }));
        break;
      case "plan.proposed":
        plansSoon();
        if (!mine && d.decision !== "auto_apply") {
          shell.toast({
            tone: "warn",
            title: `Approval needed · tier ${d.tier}`,
            body: d.prompt ?? firstLine(d.summary),
            action: { label: "Review", run: () => useShell.getState().open("approvals", { planId: d.plan_id }) },
          });
        }
        break;
      case "plan.blocked":
        plansSoon();
        if (!mine) shell.toast({ tone: "danger", title: "Refused by policy", body: d.prompt ?? firstLine(d.summary) });
        break;
      case "plan.invalid":
      case "plan.noop":
      case "plan.approved":
      case "plan.rejected":
        plansSoon();
        break;
      case "apply.started":
        set({ applying: { generation: d.generation, planId: d.plan_id } });
        break;
      case "apply.rolled_back":
        set({ applying: null });
        plansSoon();
        void get().refreshGenerations().catch(() => {});
        shell.toast({
          tone: "danger",
          title: `Rolled back to generation ${d.restored}`,
          body: (d.failed as string[] | undefined)?.join("; ") || "A health check failed, so nothing changed.",
          ttl: 12000,
        });
        break;
      case "apply.failed":
        set({ applying: null });
        plansSoon();
        void get().refreshGenerations().catch(() => {});
        shell.toast({ tone: "danger", title: `Apply failed; generation ${d.restored} kept`, body: firstLine(d.error), ttl: 12000 });
        break;
      case "generation.switched":
        set({ applying: null });
        void get()
          .refreshSystem()
          .then(() => get().refreshTheme())
          .catch(() => {});
        void get().refreshGenerations().catch(() => {});
        plansSoon();
        if (!mine) {
          shell.toast({
            tone: d.staged ? "warn" : "success",
            title: d.staged ? `Generation ${d.generation} staged for restart` : `Generation ${d.generation} is live`,
            body: `${d.theme} · ${firstLine(d.summary)}`,
          });
        }
        break;
      case "session.revoked":
        if (d.session?.id && d.session.id === useSession.getState().session?.sessionId) useSession.getState().expire();
        break;
      default:
        break;
    }
  },

  previewPlan: async (id, label, tier) => {
    const [preview, css, svg] = await Promise.all([
      api.planPreview(id),
      api.planPreviewFile(id, "etc/fluxos/theme.css"),
      api.planPreviewFile(id, "etc/fluxos/wallpaper.svg"),
    ]);
    set({ preview: { kind: "plan", id, label, spec: preview.spec, css, wallpaperUrl: svgUrl(svg), tier } });
    useShell.getState().show(null);
  },

  previewGeneration: async (n) => {
    const [g, css, svg] = await Promise.all([
      api.generation(n),
      api.generationFile(n, "etc/fluxos/theme.css"),
      api.generationFile(n, "etc/fluxos/wallpaper.svg"),
    ]);
    set({ preview: { kind: "generation", id: String(n), label: `Generation ${n} · ${g.meta.theme_name}`, spec: g.spec, css, wallpaperUrl: svgUrl(svg) } });
    useShell.getState().show(null);
  },

  endPreview: () => {
    set({ preview: null });
  },

  endRestart: () => set({ restart: null }),

  proposeUi: (prompt, operations, autoApply) =>
    api.propose({ intent: { prompt, source: "ui", session_id: clientId }, engine: { kind: "ui" }, operations }, autoApply),
}));

/** Wallpaper URL for the live system (cache-busted per generation). */
export function liveWallpaper(key: number): string {
  return fluxdUrl(`/v1/wallpaper.svg?g=${key}`);
}
