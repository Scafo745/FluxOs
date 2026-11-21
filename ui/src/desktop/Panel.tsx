import { Bell, ChevronDown, CloudOff, Eye, SlidersHorizontal } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { FluxMark } from "../components/FluxMark";
import { Spinner, StatusDot } from "../components/ui";
import { clockTime } from "../lib/format";
import type { Spec } from "../lib/types";
import { useShell } from "../state/shell";
import { selectPending, useSystem } from "../state/system";
import { DockIcon, useDockItems } from "./Dock";

function PanelButton({ children, onClick, label, active, className = "" }: { children: React.ReactNode; onClick?: () => void; label: string; active?: boolean; className?: string }) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={label}
      aria-label={label}
      className={`flex h-[calc(100%-6px)] items-center gap-1.5 rounded-md px-2 text-fg/90 transition-colors hover:bg-fg/[0.09] ${active ? "bg-fg/[0.1]" : ""} ${className}`}
    >
      {children}
    </button>
  );
}

function GlobalMenu() {
  const focused = useShell((s) => s.wins.find((w) => w.id === s.focused));
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, [open]);
  const shell = useShell.getState();
  return (
    <div ref={ref} className="relative flex h-full items-center gap-0.5">
      <span className="px-2 font-semibold">{focused ? focused.title : "Desktop"}</span>
      {focused && (
        <>
          <PanelButton label="Window menu" onClick={() => setOpen((v) => !v)} active={open}>
            Window <ChevronDown className="size-3.5 opacity-60" />
          </PanelButton>
          {open && (
            <div className="glass absolute top-full left-0 z-50 mt-1 w-48 rounded-lg border hairline p-1 text-sm shadow-raised">
              {[
                ["Minimize", () => shell.minimize(focused.id)],
                [focused.maximized ? "Restore size" : "Maximize", () => shell.toggleMax(focused.id)],
                ["Close", () => shell.close(focused.id)],
              ].map(([label, run]) => (
                <button
                  key={label as string}
                  type="button"
                  onClick={() => {
                    (run as () => void)();
                    setOpen(false);
                  }}
                  className="block w-full rounded-md px-2.5 py-1.5 text-left hover:bg-accent hover:text-on-accent"
                >
                  {label as string}
                </button>
              ))}
            </div>
          )}
        </>
      )}
    </div>
  );
}

function Workspaces({ count, numbers }: { count: number; numbers: boolean }) {
  const current = useShell((s) => s.workspace);
  const wins = useShell((s) => s.wins);
  const used = new Set(wins.map((w) => w.workspace));
  if (count <= 1) return null;
  return (
    <div className="flex items-center gap-1" role="tablist" aria-label="Workspaces">
      {Array.from({ length: count }, (_, i) => i + 1).map((n) => (
        <button
          key={n}
          type="button"
          role="tab"
          aria-selected={current === n}
          aria-label={`Workspace ${n}`}
          onClick={() => useShell.getState().setWorkspace(n)}
          className={
            numbers
              ? `grid h-5 min-w-5 place-items-center rounded px-1 font-mono text-xs transition-colors ${current === n ? "bg-accent text-on-accent" : used.has(n) ? "text-fg hover:bg-fg/10" : "text-muted hover:bg-fg/10"}`
              : `h-2 rounded-full transition-all ${current === n ? "w-5 bg-accent" : used.has(n) ? "w-2 bg-fg/60" : "w-2 bg-fg/25 hover:bg-fg/45"}`
          }
        >
          {numbers ? n : null}
        </button>
      ))}
    </div>
  );
}

function Taskbar({ spec, height }: { spec: Spec; height: number }) {
  const items = useDockItems(spec, 5);
  const size = Math.max(20, Math.min(34, height - 10));
  return (
    <nav aria-label="Taskbar" className="flex items-center" style={{ gap: Math.round(size * 0.22) }}>
      {items.map((item) => (
        <DockIcon key={item.key} item={item} size={size} edge={spec.desktop.panel.position === "top" ? "top" : "bottom"} />
      ))}
    </nav>
  );
}

function Clock() {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const t = setInterval(() => setNow(new Date()), 15000);
    return () => clearInterval(t);
  }, []);
  return (
    <span className="px-2 font-medium tabular-nums" title={now.toLocaleDateString(undefined, { weekday: "long", month: "long", day: "numeric" })}>
      {clockTime(now)}
    </span>
  );
}

function AiStatus() {
  const agent = useSystem((s) => s.agent);
  let tone: "ok" | "accent" | "bad" | "muted" = "muted";
  let label = "Connecting to the FluxOS AI";
  if (agent === "offline") {
    tone = "bad";
    label = "FluxOS AI (flux-agent) is offline";
  } else if (agent) {
    const cloud = agent.providers.filter((p) => p.circuit === "closed");
    tone = cloud.length ? "ok" : "accent";
    label = cloud.length
      ? `AI: local rules + ${cloud.map((p) => `${p.id}/${p.model}`).join(", ")}`
      : "AI: local rule engine only (add a provider key for open-ended requests)";
  }
  return (
    <PanelButton label={label} onClick={() => useShell.getState().toggle("assistant")}>
      <StatusDot tone={tone} />
      <span className="text-xs font-semibold tracking-wide">AI</span>
    </PanelButton>
  );
}

export function Panel({ spec }: { spec: Spec }) {
  const d = spec.desktop;
  const top = d.panel.position === "top";
  const h = d.panel.height_px;
  const icon = Math.round(Math.min(20, Math.max(13, h * 0.52)));
  const dockless = !d.dock.enabled;
  const centered = dockless && (d.style === "snap-layouts" || h >= 40);
  const numbers = d.windows.tiling || d.style === "ops-center";
  const pending = useSystem((s) => selectPending(s).length);
  const system = useSystem((s) => s.system);
  const applying = useSystem((s) => s.applying);
  const conn = useSystem((s) => s.conn);
  const preview = useSystem((s) => s.preview);
  const overlay = useShell((s) => s.overlay);
  const shell = useShell.getState();

  return (
    <header
      className={`glass fixed inset-x-0 z-40 flex items-center px-1.5 text-sm ${top ? "chrome-down top-0 border-b" : "chrome-up bottom-0 border-t"} hairline`}
      style={{ height: h }}
    >
      <div className="flex h-full min-w-0 flex-1 items-center gap-1">
        <PanelButton label="Ask Flux (Ctrl+K)" onClick={() => shell.toggle("assistant")} active={overlay === "assistant"}>
          <FluxMark size={icon + 4} />
          {h >= 26 && <span className="font-display font-bold tracking-tight">FluxOS</span>}
        </PanelButton>
        {d.panel.global_menu && <GlobalMenu />}
        {dockless && !centered && <Taskbar spec={spec} height={h} />}
        {!centered && !d.panel.global_menu && <div className="px-1"><Workspaces count={d.workspaces} numbers={numbers} /></div>}
      </div>

      {centered ? (
        <div className="absolute left-1/2 flex h-full -translate-x-1/2 items-center">
          <Taskbar spec={spec} height={h} />
        </div>
      ) : (
        d.panel.global_menu && (
          <div className="absolute left-1/2 flex h-full -translate-x-1/2 items-center">
            <Workspaces count={d.workspaces} numbers={numbers} />
          </div>
        )
      )}

      <div className="flex h-full flex-1 items-center justify-end gap-0.5">
        {centered && <div className="px-2"><Workspaces count={d.workspaces} numbers={numbers} /></div>}
        {preview && (
          <span className="flex items-center gap-1.5 rounded-md bg-accent/15 px-2 py-0.5 text-xs font-semibold text-accent">
            <Eye className="size-3.5" /> Preview
          </span>
        )}
        {applying && (
          <span className="flex items-center gap-1.5 rounded-md bg-fg/[0.08] px-2 py-0.5 text-xs">
            <Spinner className="size-3.5" /> Applying gen {applying.generation}
          </span>
        )}
        {conn === "offline" && (
          <span className="flex items-center gap-1.5 rounded-md bg-bad/15 px-2 py-0.5 text-xs font-semibold text-bad" title="Lost the connection to fluxd; retrying">
            <CloudOff className="size-3.5" /> Offline
          </span>
        )}
        <AiStatus />
        <PanelButton label={pending ? `${pending} change(s) waiting for approval` : "Approvals"} onClick={() => shell.open("approvals")}>
          <span className="relative">
            <Bell style={{ width: icon, height: icon }} />
            {pending > 0 && <span className="absolute -top-1 -right-1.5 grid h-3.5 min-w-3.5 place-items-center rounded-full bg-bad px-0.5 text-[9px] font-bold text-white">{pending}</span>}
          </span>
        </PanelButton>
        {system && (
          <PanelButton label="Generation timeline" onClick={() => shell.open("timeline")}>
            <span className={`size-1.5 rounded-full ${system.status === "staged" ? "bg-warn" : "bg-ok"}`} />
            <span className="font-mono text-xs">gen {system.generation}</span>
          </PanelButton>
        )}
        <Clock />
        <PanelButton label="Quick settings" onClick={() => shell.toggle("control")} active={overlay === "control"}>
          <SlidersHorizontal style={{ width: icon, height: icon }} />
        </PanelButton>
      </div>
    </header>
  );
}
