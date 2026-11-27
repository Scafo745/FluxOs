import { Package, Search, Sparkles, Trash } from "lucide-react";
import { useMemo, useState } from "react";

import { APPS, DOCK_APPS, TINT_BG } from "../apps/meta";
import { Button, Chip, TierBadge } from "../components/ui";
import { category, hueGradient, installedApps, type InstalledApp } from "../lib/apps";
import type { Spec } from "../lib/types";
import { errorText, reportPlan } from "../state/actions";
import { useAssistant } from "../state/assistant";
import { useShell } from "../state/shell";
import { useSystem } from "../state/system";

function Tile({ label, sub, background, icon: Icon, onClick, active }: { label: string; sub?: string; background: string; icon: typeof Package; onClick: () => void; active?: boolean }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`group flex flex-col items-center gap-2 rounded-xl p-3 text-center transition-colors hover:bg-fg/[0.07] ${active ? "bg-fg/[0.1]" : ""}`}
    >
      <span
        className="grid size-14 place-items-center rounded-[28%] text-white shadow-[0_8px_20px_-8px_rgb(0_0_0/0.6),inset_0_1px_0_rgb(255_255_255/0.25)] transition-transform group-hover:scale-105"
        style={{ background }}
      >
        <Icon className="size-7" strokeWidth={2} />
      </span>
      <span className="line-clamp-1 text-sm font-medium">{label}</span>
      {sub && <span className="-mt-1.5 line-clamp-1 text-xs text-muted">{sub}</span>}
    </button>
  );
}

/** All installed software, from the spec (FluxOS apps open; Linux apps show details). */
export function Launcher({ spec }: { spec: Spec }) {
  const open = useShell((s) => s.overlay === "launcher");
  const catalog = useSystem((s) => s.catalog);
  const [q, setQ] = useState("");
  const [selected, setSelected] = useState<InstalledApp | null>(null);
  const [busy, setBusy] = useState(false);
  const apps = useMemo(() => installedApps(spec, catalog), [spec, catalog]);
  if (!open) return null;

  const match = (s: string) => s.toLowerCase().includes(q.trim().toLowerCase());
  const shown = apps.filter((a) => !q || match(a.name) || match(a.id) || match(a.category));
  const system = DOCK_APPS.filter((id) => !q || match(APPS[id].title));
  const services = spec.services.enabled.filter((s) => !q || match(s));

  async function remove(app: InstalledApp) {
    setBusy(true);
    try {
      const plan = await useSystem
        .getState()
        .proposeUi(`Remove ${app.name}`, [app.source === "flatpak" ? { tool: "apps.remove", params: { apps: [app.id] } } : { tool: "packages.remove", params: { packages: [app.id] } }], true);
      reportPlan(plan);
      setSelected(null);
    } catch (e) {
      useShell.getState().toast({ tone: "danger", title: "Could not remove", body: errorText(e) });
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="fixed inset-0 z-[55] bg-bg/55 backdrop-blur-md fade-in" onMouseDown={(e) => e.target === e.currentTarget && useShell.getState().show(null)}>
      <div className="mx-auto flex h-full max-w-5xl flex-col px-6 pt-[9vh] pb-8" onMouseDown={(e) => e.target === e.currentTarget && useShell.getState().show(null)}>
        <label className="glass mx-auto flex w-[min(520px,100%)] items-center gap-2 rounded-full border hairline px-4 py-2.5 shadow-raised">
          <Search className="size-4 text-muted" />
          <input autoFocus value={q} onChange={(e) => setQ(e.target.value)} placeholder="Search apps and services" className="flex-1 bg-transparent text-base outline-none placeholder:text-muted" />
        </label>

        <div className="mt-8 min-h-0 flex-1 overflow-y-auto">
          {system.length > 0 && (
            <>
              <p className="mb-2 px-3 text-xs font-semibold tracking-[0.14em] text-muted uppercase">FluxOS</p>
              <div className="grid grid-cols-[repeat(auto-fill,minmax(120px,1fr))] gap-1">
                <Tile label="Ask Flux" sub="AI assistant" background="linear-gradient(140deg, var(--flux-color-accent), var(--flux-color-secondary))" icon={Sparkles} onClick={() => useShell.getState().show("assistant")} />
                {system.map((id) => (
                  <Tile key={id} label={APPS[id].title} sub={APPS[id].blurb} background={TINT_BG[APPS[id].tint]} icon={APPS[id].icon} onClick={() => useShell.getState().open(id)} />
                ))}
              </div>
            </>
          )}

          <p className="mt-6 mb-2 px-3 text-xs font-semibold tracking-[0.14em] text-muted uppercase">Installed on this system ({shown.length})</p>
          <div className="grid grid-cols-[repeat(auto-fill,minmax(120px,1fr))] gap-1">
            {shown.map((a) => {
              const cat = category(a.category);
              return (
                <Tile
                  key={a.id}
                  label={a.name}
                  sub={a.source === "flatpak" ? "Flathub" : "Nix"}
                  background={hueGradient(cat.hue)}
                  icon={cat.icon}
                  active={selected?.id === a.id}
                  onClick={() => setSelected(a)}
                />
              );
            })}
            <Tile
              label="Install apps"
              sub="Ask Flux"
              background="color-mix(in oklab, var(--flux-color-text) 14%, transparent)"
              icon={Package}
              onClick={() => {
                useAssistant.getState().setDraft("install ");
                useShell.getState().show("assistant");
              }}
            />
          </div>

          {services.length > 0 && (
            <>
              <p className="mt-6 mb-2 px-3 text-xs font-semibold tracking-[0.14em] text-muted uppercase">Services</p>
              <div className="flex flex-wrap gap-1.5 px-3">
                {services.map((s) => (
                  <Chip key={s}>{s}</Chip>
                ))}
              </div>
            </>
          )}
        </div>

        {selected && (
          <div className="glass mx-auto mt-4 flex w-[min(720px,100%)] items-center gap-4 rounded-xl border hairline p-4 shadow-raised rise-in">
            <span className="grid size-12 shrink-0 place-items-center rounded-[28%] text-white" style={{ background: hueGradient(category(selected.category).hue) }}>
              {(() => {
                const Icon = category(selected.category).icon;
                return <Icon className="size-6" />;
              })()}
            </span>
            <div className="min-w-0 flex-1">
              <p className="font-semibold">{selected.name}</p>
              <p className="truncate text-sm text-muted">{selected.description}</p>
              <p className="mt-1 text-xs text-muted">
                {selected.source === "flatpak" ? "Sandboxed Flathub app" : "System package from nixpkgs"} · {category(selected.category).label}
              </p>
            </div>
            <TierBadge tier={selected.source === "flatpak" ? 1 : 2} />
            <Button variant="danger" icon={Trash} busy={busy} onClick={() => void remove(selected)}>
              Remove
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}
