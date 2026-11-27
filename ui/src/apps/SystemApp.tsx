import { FileCode, LogOut, MonitorSmartphone, ShieldCheck } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { CodeView } from "../components/CodeView";
import { ArrowText, Button, Chip, Empty, KV, Section, Swatch, Tabs } from "../components/ui";
import { api } from "../lib/api";
import { contrast, contrastGrade } from "../lib/color";
import { ago, desktopLabel, shortHash, titleCase } from "../lib/format";
import type { SessionInfo, Spec } from "../lib/types";
import { errorText } from "../state/actions";
import { useSession } from "../state/session";
import { useShell } from "../state/shell";
import { useSystem } from "../state/system";

type Tab = "overview" | "design" | "config" | "screens";

function Card({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="card p-4">
      <p className="mb-2.5 text-xs font-semibold tracking-[0.12em] text-muted uppercase">{title}</p>
      {children}
    </div>
  );
}

function Overview({ spec }: { spec: Spec }) {
  const system = useSystem((s) => s.system);
  const catalog = useSystem((s) => s.catalog);
  const kernel = catalog?.kernels.find((k) => k.variant === spec.kernel.variant);
  const sched = catalog?.schedulers.find((s) => s.id === spec.kernel.sched_ext);
  const fw = spec.security.firewall;
  const ports = [...fw.allowed_tcp_ports.map((p) => `${p}/tcp`), ...fw.allowed_udp_ports.map((p) => `${p}/udp`)];
  if (!system) return null;
  return (
    <div className="space-y-4 p-5">
      <div className="flex flex-wrap items-end gap-x-6 gap-y-2">
        <div>
          <p className="text-xs tracking-[0.14em] text-muted uppercase">{spec.base.hostname}</p>
          <h2 className="font-display text-2xl font-bold">{spec.design.theme_name}</h2>
        </div>
        <p className="pb-1 text-sm text-muted">
          Generation {system.generation} · {system.status} · {system.target.kind} target · {system.mode} mode · audit #{system.audit_head[0]}
        </p>
      </div>
      <div className="grid gap-3 md:grid-cols-2">
        <Card title="Kernel and scheduling">
          <KV
            rows={[
              ["Kernel", `${kernel?.label ?? spec.kernel.variant} · ${kernel?.version_hint ?? ""}`],
              ["Scheduler", sched?.binary ? `${sched.binary} (sched_ext, ${sched.maturity})` : "kernel default (EEVDF)"],
              ["CPU governor", spec.kernel.cpu_governor],
              ["I/O scheduler", spec.kernel.io_scheduler],
              ["sysctl", Object.keys(spec.kernel.sysctl).length ? `${Object.keys(spec.kernel.sysctl).length} tuned keys` : "defaults"],
            ]}
          />
        </Card>
        <Card title="Desktop">
          <KV
            rows={[
              ["Environment", desktopLabel(spec.desktop.environment)],
              ["Style", titleCase(spec.desktop.style)],
              ["Dock", spec.desktop.dock.enabled ? `${spec.desktop.dock.position}${spec.desktop.dock.autohide ? ", auto-hide" : ""}` : "off"],
              ["Windows", spec.desktop.windows.tiling ? `tiling, ${spec.desktop.windows.gaps_px}px gaps` : spec.desktop.windows.snapping ? "floating with snapping" : "floating"],
              ["Workspaces", String(spec.desktop.workspaces)],
            ]}
          />
        </Card>
        <Card title="Security">
          <KV
            rows={[
              ["Firewall", ports.length ? `on · open ${ports.join(", ")}` : "on · no inbound ports"],
              ["Access control", spec.security.mac === "apparmor" ? "AppArmor" : "off"],
              ["USB storage", spec.security.usb_storage],
              ["Auto-lock", spec.security.screen_lock.enabled ? `${spec.security.screen_lock.idle_minutes} min` : "off"],
              ["Audit", spec.security.audit ? "on, hash-chained log" : "off"],
            ]}
          />
        </Card>
        <Card title="Session and accessibility">
          <KV
            rows={[
              ["Session", spec.session.mode === "standard" ? "standard desktop" : <ArrowText text={`${spec.session.mode} → ${spec.session.kiosk_url}`} />],
              ["Shared account", spec.session.ephemeral_home ? "wiped on every boot" : "off"],
              ["Contrast", spec.accessibility.high_contrast ? "high (WCAG AAA)" : "normal"],
              ["Text", `${Math.round((spec.accessibility.large_text ? Math.max(1.25, spec.accessibility.text_scale) : spec.accessibility.text_scale) * 100)}%`],
              ["Motion", spec.accessibility.reduce_motion ? "reduced" : "full"],
            ]}
          />
        </Card>
        <Card title="Hardware">
          <p className="text-sm">{system.hardware}</p>
          <p className="mt-2 text-xs text-muted">GPU driver: {spec.drivers.gpu} · controllers {spec.drivers.gaming_controllers ? "on" : "off"}</p>
        </Card>
        <Card title="Software">
          <p className="text-sm">
            {spec.packages.system.length} system packages · {spec.packages.flatpak.length} Flathub apps · {spec.services.enabled.length} services
          </p>
          <div className="mt-2 flex flex-wrap gap-1">
            {spec.services.enabled.map((s) => (
              <Chip key={s}>{s}</Chip>
            ))}
          </div>
        </Card>
      </div>
      <p className="text-xs text-muted">Spec digest {shortHash(system.spec_digest, 16)}</p>
    </div>
  );
}

function Easing({ curve }: { curve: [number, number, number, number] }) {
  const [x1, y1, x2, y2] = curve;
  return (
    <svg viewBox="-10 -30 120 160" className="h-36 w-28" aria-label={`cubic-bezier(${curve.join(", ")})`}>
      <rect x="0" y="0" width="100" height="100" fill="none" stroke="var(--flux-color-border)" />
      <line x1="0" y1="100" x2={x1 * 100} y2={100 - y1 * 100} stroke="var(--flux-color-text-muted)" strokeDasharray="3 3" />
      <line x1="100" y1="0" x2={x2 * 100} y2={100 - y2 * 100} stroke="var(--flux-color-text-muted)" strokeDasharray="3 3" />
      <path d={`M0 100 C ${x1 * 100} ${100 - y1 * 100}, ${x2 * 100} ${100 - y2 * 100}, 100 0`} fill="none" stroke="var(--flux-color-accent)" strokeWidth="3" strokeLinecap="round" />
    </svg>
  );
}

function Design({ spec }: { spec: Spec }) {
  const t = spec.design.tokens;
  const p = t.palette;
  const entries = Object.entries(p) as [keyof typeof p, string][];
  const ty = t.typography;
  return (
    <div className="space-y-6 p-5">
      <Section title="Palette" right={<span className="text-xs text-muted">contrast against background, WCAG 2.x</span>}>
        <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-4">
          {entries.map(([name, hex]) => {
            const ratio = contrast(hex, p.background);
            return (
              <div key={name} className="flex items-center gap-3 rounded-lg border hairline p-2.5">
                <Swatch hex={hex} size={34} />
                <div className="min-w-0">
                  <p className="truncate text-sm font-medium">{titleCase(name)}</p>
                  <p className="font-mono text-xs text-muted">
                    {hex} · {name === "background" ? "base" : `${ratio.toFixed(1)}:1 ${contrastGrade(ratio)}`}
                  </p>
                </div>
              </div>
            );
          })}
        </div>
      </Section>
      <Section title="Typography">
        <div className="grid gap-3 md:grid-cols-3">
          <div className="card p-4">
            <p className="text-xs text-muted">Display · {ty.font_display}</p>
            <p className="mt-1 font-display text-3xl font-bold">Aa Flux</p>
          </div>
          <div className="card p-4">
            <p className="text-xs text-muted">
              Interface · {ty.font_sans} {ty.base_size_px}px · ×{ty.scale_ratio}
            </p>
            <p className="mt-1 text-sm">Describe the computer you need, and get it. Every change is validated, reversible and logged.</p>
          </div>
          <div className="card p-4">
            <p className="text-xs text-muted">Monospace · {ty.font_mono}</p>
            <p className="mt-1 font-mono text-sm">boot.kernelPackages = pkgs.linuxPackages_zen;</p>
          </div>
        </div>
      </Section>
      <div className="grid gap-6 md:grid-cols-2">
        <Section title="Shape">
          <div className="flex items-end gap-4">
            {(["radius_sm_px", "radius_md_px", "radius_lg_px"] as const).map((k) => (
              <div key={k} className="text-center">
                <div className="size-16 border-2 border-accent bg-accent/15" style={{ borderRadius: t.shape[k] }} />
                <p className="mt-1 font-mono text-xs text-muted">{t.shape[k]}px</p>
              </div>
            ))}
            <div className="text-center">
              <div className="h-16 w-24 border hairline bg-surface" style={{ borderRadius: spec.desktop.windows.corner_radius_px }} />
              <p className="mt-1 font-mono text-xs text-muted">window {spec.desktop.windows.corner_radius_px}px</p>
            </div>
          </div>
        </Section>
        <Section title="Motion and effects">
          <div className="flex items-center gap-5">
            <Easing curve={t.motion.easing} />
            <KV
              rows={[
                ["Fast", `${t.motion.duration_fast_ms} ms`],
                ["Standard", `${t.motion.duration_base_ms} ms`],
                ["Slow", `${t.motion.duration_slow_ms} ms`],
                ["Scale", `×${t.motion.animation_scale}`],
                ["Blur", `${t.effects.blur_px}px · ${Math.round(t.effects.transparency * 100)}% translucent`],
              ]}
            />
          </div>
        </Section>
      </div>
      <p className="text-xs text-muted">
        The same tokens are exported as W3C DTCG JSON, CSS, GTK, KDE color schemes, GNOME dconf and Hyprland config. See the Config tab.
      </p>
    </div>
  );
}

function Config() {
  const system = useSystem((s) => s.system);
  const gens = useSystem((s) => s.generations);
  const meta = gens.find((g) => g.number === system?.generation);
  const files = useMemo(() => meta?.artifacts ?? [], [meta]);
  const [path, setPath] = useState("etc/nixos/fluxos/generated.nix");
  const [text, setText] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!system) return;
    setText(null);
    setError(null);
    api
      .generationFile(system.generation, path)
      .then(setText)
      .catch((e) => setError(errorText(e)));
  }, [system, path]);
  if (!system) return null;
  return (
    <div className="flex h-full">
      <aside className="w-64 shrink-0 overflow-y-auto border-r hairline p-2">
        <p className="px-2 pt-2 pb-1 text-xs font-semibold tracking-[0.12em] text-muted uppercase">Generation {system.generation}</p>
        {files.map((f) => (
          <button
            key={f.path}
            type="button"
            onClick={() => setPath(f.path)}
            className={`block w-full truncate rounded-md px-2 py-1.5 text-left font-mono text-xs ${f.path === path ? "bg-accent/15 text-fg" : "text-muted hover:bg-fg/[0.05] hover:text-fg"}`}
            title={`${f.path} · ${f.bytes} bytes · sha256 ${f.sha256.slice(0, 12)}`}
          >
            {f.path.replace(/^etc\/|^usr\/share\//, "")}
          </button>
        ))}
      </aside>
      <div className="min-w-0 flex-1 overflow-auto">
        <CodeView text={text} path={path} error={error} />
      </div>
    </div>
  );
}

function Screens() {
  const session = useSession((s) => s.session);
  const [list, setList] = useState<SessionInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const load = () =>
    api
      .sessions()
      .then((r) => setList(r.sessions))
      .catch((e) => setError(errorText(e)));
  useEffect(() => {
    void load();
  }, []);
  return (
    <div className="space-y-4 p-5">
      <p className="text-sm text-muted">Screens paired with this machine. Each holds its own revocable token; fluxd stores only a hash of it.</p>
      {error && <p className="text-sm text-bad">{error}</p>}
      {list && list.length === 0 && <Empty icon={MonitorSmartphone} title="No paired screens" />}
      <ul className="space-y-2">
        {(list ?? []).map((s) => (
          <li key={s.id} className="flex items-center gap-3 rounded-lg border hairline p-3">
            <MonitorSmartphone className="size-5 text-muted" />
            <div className="min-w-0 flex-1">
              <p className="font-medium">
                {s.name} {s.id === session?.sessionId && <span className="text-xs text-accent">(this screen)</span>}
              </p>
              <p className="text-xs text-muted">
                {s.role === "admin" ? "owner" : s.role} · paired {ago(s.created_at)} · {s.id}
              </p>
            </div>
            {s.id === session?.sessionId ? (
              <Button
                size="sm"
                icon={LogOut}
                onClick={async () => {
                  try {
                    await api.logout();
                  } finally {
                    useSession.getState().clear();
                  }
                }}
              >
                Sign out
              </Button>
            ) : (
              <Button
                size="sm"
                variant="danger"
                onClick={async () => {
                  try {
                    await api.revokeSession(s.id);
                    void load();
                  } catch (e) {
                    useShell.getState().toast({ tone: "danger", title: "Could not revoke", body: errorText(e) });
                  }
                }}
              >
                Revoke
              </Button>
            )}
          </li>
        ))}
      </ul>
      <p className="flex items-center gap-2 text-xs text-muted">
        <ShieldCheck className="size-4 text-ok" /> Pair another screen with a new one-time code: <code className="font-mono">scripts/flux pair</code> or{" "}
        <code className="font-mono">sudo fluxd pair-code</code>.
      </p>
    </div>
  );
}

export function SystemApp() {
  const spec = useSystem((s) => s.system?.spec);
  const [tab, setTab] = useState<Tab>("overview");
  if (!spec) return <Empty icon={FileCode} title="Loading" />;
  return (
    <div className="flex h-full flex-col">
      <div className="border-b hairline px-5 py-3">
        <Tabs
          value={tab}
          onChange={setTab}
          options={[
            { id: "overview", label: "Overview" },
            { id: "design", label: "Design tokens" },
            { id: "config", label: "Generated config" },
            { id: "screens", label: "Paired screens" },
          ]}
        />
      </div>
      <div className="min-h-0 flex-1 overflow-auto">
        {tab === "overview" && <Overview spec={spec} />}
        {tab === "design" && <Design spec={spec} />}
        {tab === "config" && <Config />}
        {tab === "screens" && <Screens />}
      </div>
    </div>
  );
}
