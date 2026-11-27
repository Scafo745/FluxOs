import { Download, Layers, Save, ShieldCheck, Upload } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { Button, Chip, Empty } from "../components/ui";
import { api } from "../lib/api";
import { desktopLabel, titleCase } from "../lib/format";
import type { ProfileDetail, ProfileSummary, Spec } from "../lib/types";
import { errorText, reportPlan } from "../state/actions";
import { useShell } from "../state/shell";
import { useSystem } from "../state/system";

/** A thumbnail of the desktop a profile produces, drawn from its spec. */
function MiniDesktop({ spec, accent }: { spec?: Spec; accent: string }) {
  if (!spec) return <div className="h-36 animate-pulse rounded-lg" style={{ background: `linear-gradient(135deg, ${accent}55, transparent)` }} />;
  const p = spec.design.tokens.palette;
  const d = spec.desktop;
  const radius = Math.max(2, d.windows.corner_radius_px / 2);
  const panelH = Math.max(6, Math.round(d.panel.height_px / 4));
  const win = (style: React.CSSProperties, focused = false) => (
    <div className="absolute overflow-hidden" style={{ ...style, background: p.surface, border: `1px solid ${focused && d.windows.tiling ? p.accent : p.border}`, borderRadius: radius }}>
      <div className="flex h-2.5 items-center gap-0.5 px-1" style={{ background: p.surface_raised }}>
        <span className="size-1 rounded-full" style={{ background: p.accent }} />
      </div>
      <div className="space-y-1 p-1.5">
        <div className="h-1 w-3/4 rounded" style={{ background: p.text, opacity: 0.6 }} />
        <div className="h-1 w-1/2 rounded" style={{ background: p.text_muted, opacity: 0.6 }} />
        <div className="mt-1.5 h-2 w-8 rounded" style={{ background: p.accent }} />
      </div>
    </div>
  );
  const dockItems = [p.accent, p.secondary, p.success, p.warning, p.text_muted];
  return (
    <div
      className="relative h-36 overflow-hidden rounded-lg"
      style={{ background: `radial-gradient(circle at 22% 18%, ${p.accent}66, transparent 55%), radial-gradient(circle at 82% 90%, ${p.secondary}55, transparent 52%), ${p.background}` }}
    >
      <div className="absolute inset-x-0 flex items-center gap-1 px-1.5" style={{ [d.panel.position]: 0, height: panelH, background: `${p.surface}e6` }}>
        <span className="size-1.5 rounded-full" style={{ background: p.accent }} />
        {!d.dock.enabled && dockItems.slice(0, 4).map((c, i) => <span key={i} className="size-1.5 rounded-sm" style={{ background: c }} />)}
      </div>
      {d.windows.tiling ? (
        <>
          {win({ left: "6%", top: d.panel.position === "top" ? "18%" : "6%", width: "50%", height: "70%" }, true)}
          {win({ left: "58%", top: d.panel.position === "top" ? "18%" : "6%", width: "36%", height: "33%" })}
          {win({ left: "58%", top: d.panel.position === "top" ? "55%" : "43%", width: "36%", height: "33%" })}
        </>
      ) : (
        <>
          {win({ left: "12%", top: "22%", width: "46%", height: "50%" })}
          {win({ left: "40%", top: "32%", width: "44%", height: "46%" }, true)}
        </>
      )}
      {d.dock.enabled && (
        <div
          className="absolute flex gap-1 p-1"
          style={{
            background: `${p.surface}d9`,
            borderRadius: 6,
            ...(d.dock.position === "bottom"
              ? { bottom: d.panel.position === "bottom" ? panelH + 4 : 5, left: "50%", transform: "translateX(-50%)" }
              : { [d.dock.position]: 5, top: "50%", transform: "translateY(-50%)", flexDirection: "column" as const }),
          }}
        >
          {dockItems.map((c, i) => (
            <span key={i} className="size-2.5 rounded-[3px]" style={{ background: c }} />
          ))}
        </div>
      )}
    </div>
  );
}

function ProfileCard({ p }: { p: ProfileSummary }) {
  const [detail, setDetail] = useState<ProfileDetail | null>(null);
  const [busy, setBusy] = useState<"apply" | "export" | null>(null);
  useEffect(() => {
    api
      .profile(p.id)
      .then(setDetail)
      .catch(() => setDetail(null));
  }, [p.id]);
  const display = detail?.spec.design.tokens.typography.font_display;

  async function apply() {
    setBusy("apply");
    try {
      const plan = await useSystem.getState().proposeUi(`Apply the ${p.name} profile`, [{ tool: "profile.apply", params: { profile: p.id } }], false);
      if (plan.status === "proposed") useShell.getState().open("approvals", { planId: plan.id });
      else reportPlan(plan);
    } catch (e) {
      useShell.getState().toast({ tone: "danger", title: "Could not propose", body: errorText(e) });
    } finally {
      setBusy(null);
    }
  }

  async function exportBundle() {
    setBusy("export");
    try {
      const bundle = await api.exportProfile(p.id);
      const url = URL.createObjectURL(new Blob([JSON.stringify(bundle, null, 2)], { type: "application/json" }));
      const a = document.createElement("a");
      a.href = url;
      a.download = `${p.id}.fluxprofile.json`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      useShell.getState().toast({ tone: "danger", title: "Export failed", body: errorText(e) });
    } finally {
      setBusy(null);
    }
  }

  return (
    <article className="card flex flex-col overflow-hidden p-2.5">
      <MiniDesktop spec={detail?.spec} accent={p.accent} />
      <div className="flex flex-1 flex-col px-1.5 pt-3 pb-1">
        <div className="flex flex-wrap items-center gap-2">
          <h3 className="text-lg leading-tight font-bold" style={display ? { fontFamily: `"${display}", var(--flux-font-display)` } : undefined}>
            {p.name}
          </h3>
          {p.status === "preview" && <Chip className="border-warn/40 text-warn">Preview</Chip>}
          {p.source !== "builtin" && <Chip>{p.source === "user" ? "Saved" : "Imported"}</Chip>}
        </div>
        <p className="mt-0.5 text-xs text-muted">
          {p.theme_name} · {desktopLabel(p.desktop)} · {titleCase(p.style)} · {p.kernel} kernel
        </p>
        <p className="mt-2 line-clamp-3 text-sm text-fg/85">{p.summary}</p>
        {p.compliance_review_required && (
          <p className="mt-2 flex items-center gap-1.5 text-xs text-warn">
            <ShieldCheck className="size-3.5" /> Needs your compliance team's review before production use.
          </p>
        )}
        <div className="mt-2 flex flex-wrap gap-1">
          {p.tags.slice(0, 4).map((t) => (
            <Chip key={t}>{t}</Chip>
          ))}
        </div>
        <div className="mt-auto flex gap-2 pt-3">
          <Button size="sm" variant="primary" busy={busy === "apply"} onClick={() => void apply()}>
            Review and apply
          </Button>
          <Button size="sm" variant="ghost" icon={Download} busy={busy === "export"} onClick={() => void exportBundle()}>
            Export
          </Button>
        </div>
      </div>
    </article>
  );
}

export function Profiles() {
  const profiles = useSystem((s) => s.profiles);
  const [saving, setSaving] = useState(false);
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const file = useRef<HTMLInputElement>(null);

  async function save() {
    const id = name
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 40);
    if (!id) return;
    setBusy(true);
    try {
      await api.saveProfile(id, name.trim());
      await useSystem.getState().loadProfiles();
      setSaving(false);
      setName("");
      useShell.getState().toast({ tone: "success", title: "Profile saved", body: `"${name.trim()}" can now be applied, exported and shared.` });
    } catch (e) {
      useShell.getState().toast({ tone: "danger", title: "Could not save", body: errorText(e) });
    } finally {
      setBusy(false);
    }
  }

  async function importFile(f: File) {
    try {
      const bundle: unknown = JSON.parse(await f.text());
      const p = await api.importProfile(bundle);
      await useSystem.getState().loadProfiles();
      useShell.getState().toast({ tone: "success", title: "Profile imported", body: `${p.name}: signature verified.` });
    } catch (e) {
      useShell.getState().toast({ tone: "danger", title: "Import refused", body: errorText(e), ttl: 10000 });
    }
  }

  if (!profiles.length) return <Empty icon={Layers} title="No profiles" />;
  return (
    <div className="p-5">
      <div className="mb-4 flex flex-wrap items-center gap-2">
        <p className="min-w-0 flex-1 text-sm text-muted">One engine, many industries. Applying a profile is a reviewed plan; your apps and open ports are kept.</p>
        {saving ? (
          <form
            className="flex gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              void save();
            }}
          >
            <input
              autoFocus
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Name this setup"
              className="h-9 w-48 rounded-md border hairline bg-bg/50 px-3 text-sm outline-none focus:border-accent"
            />
            <Button type="submit" variant="primary" busy={busy} disabled={!name.trim()}>
              Save
            </Button>
            <Button variant="ghost" onClick={() => setSaving(false)}>
              Cancel
            </Button>
          </form>
        ) : (
          <>
            <Button icon={Save} onClick={() => setSaving(true)}>
              Save current setup
            </Button>
            <Button icon={Upload} onClick={() => file.current?.click()}>
              Import signed profile
            </Button>
            <input
              ref={file}
              type="file"
              accept=".json,application/json"
              className="hidden"
              onChange={(e) => {
                const f = e.target.files?.[0];
                if (f) void importFile(f);
                e.target.value = "";
              }}
            />
          </>
        )}
      </div>
      <div className="grid grid-cols-[repeat(auto-fill,minmax(280px,1fr))] gap-3">
        {profiles.map((p) => (
          <ProfileCard key={p.id} p={p} />
        ))}
      </div>
    </div>
  );
}
