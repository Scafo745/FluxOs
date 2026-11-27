import { Ban, Cpu, Gauge, HardDrive, MemoryStick, Search, Timer } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import { Button, Empty, Spinner } from "../components/ui";
import { api } from "../lib/api";
import { bytes, duration } from "../lib/format";
import type { ProcessInfo, ProcessList, UserSession } from "../lib/types";
import { errorText } from "../state/actions";
import { useShell } from "../state/shell";

interface Row extends ProcessInfo {
  cpu: number;
}

function Meter({ icon: Icon, label, value, detail, ratio }: { icon: typeof Cpu; label: string; value: string; detail?: string; ratio?: number }) {
  return (
    <div className="card min-w-0 flex-1 p-3">
      <p className="flex items-center gap-1.5 text-xs text-muted">
        <Icon className="size-3.5" /> {label}
      </p>
      <p className="mt-1 text-lg font-semibold tabular-nums">{value}</p>
      {ratio !== undefined && (
        <div className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-fg/10">
          <div className="h-full rounded-full bg-accent transition-[width] duration-500" style={{ width: `${Math.min(100, Math.max(2, ratio * 100))}%` }} />
        </div>
      )}
      {detail && <p className="mt-1 truncate text-xs text-muted">{detail}</p>}
    </div>
  );
}

/**
 * Processes and resources of the machine, from /proc through flux-session.
 * Only the console user's own processes can be stopped; the kernel refuses the rest.
 */
export function Tasks() {
  const [list, setList] = useState<ProcessList | null>(null);
  const [info, setInfo] = useState<UserSession | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [q, setQ] = useState("");
  const [mine, setMine] = useState(true);
  const [sort, setSort] = useState<"cpu" | "rss" | "pid">("cpu");
  const [selected, setSelected] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const prev = useRef<{ total: number; ticks: Map<number, number> } | null>(null);
  const [cpu, setCpu] = useState<Map<number, number>>(new Map());

  useEffect(() => {
    let stop = false;
    const tick = async () => {
      try {
        const [p, i] = await Promise.all([api.processes(), api.userSession()]);
        if (stop) return;
        const ticks = new Map(p.processes.map((x) => [x.pid, x.cpu_ticks]));
        if (prev.current) {
          const dt = p.total_ticks - prev.current.total;
          const next = new Map<number, number>();
          for (const x of p.processes) {
            const before = prev.current.ticks.get(x.pid);
            if (before !== undefined && dt > 0) next.set(x.pid, ((x.cpu_ticks - before) / dt) * 100);
          }
          setCpu(next);
        }
        prev.current = { total: p.total_ticks, ticks };
        setList(p);
        setInfo(i);
        setError(null);
      } catch (e) {
        if (!stop) setError(errorText(e));
      }
    };
    void tick();
    const t = setInterval(() => void tick(), 2000);
    return () => {
      stop = true;
      clearInterval(t);
    };
  }, []);

  const rows: Row[] = useMemo(() => {
    if (!list) return [];
    const needle = q.trim().toLowerCase();
    return list.processes
      .filter((p) => !mine || p.uid === list.uid)
      .filter((p) => !needle || p.name.toLowerCase().includes(needle) || p.cmd.toLowerCase().includes(needle) || String(p.pid) === needle)
      .map((p) => ({ ...p, cpu: cpu.get(p.pid) ?? 0 }))
      .sort((a, b) => (sort === "pid" ? a.pid - b.pid : sort === "rss" ? b.rss - a.rss : b.cpu - a.cpu || b.rss - a.rss));
  }, [list, q, mine, sort, cpu]);

  if (error && !list) {
    return (
      <Empty icon={Gauge} title="Processes are not available">
        {error}
      </Empty>
    );
  }
  if (!list) {
    return (
      <p className="flex items-center gap-2 p-5 text-sm text-muted">
        <Spinner /> Reading /proc…
      </p>
    );
  }

  const totalCpu = Array.from(cpu.values()).reduce((a, b) => a + b, 0);
  const memTotal = (info?.memory?.total_kb ?? 0) * 1024;
  const memUsed = memTotal - (info?.memory?.available_kb ?? 0) * 1024;
  const sel = rows.find((r) => r.pid === selected);
  const ownSelected = sel && sel.uid === list.uid;

  async function stop(signal: "TERM" | "KILL") {
    if (!sel) return;
    setBusy(true);
    try {
      const r = await api.signal(sel.pid, signal);
      useShell.getState().toast({ tone: "success", title: signal === "KILL" ? "Force quit" : "Asked to quit", body: `${sel.name} (${r.message})` });
    } catch (e) {
      useShell.getState().toast({ tone: "danger", title: "Could not stop the process", body: errorText(e) });
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex flex-wrap gap-2 border-b hairline p-3">
        <Meter icon={Cpu} label={`CPU · ${list.cpus} cores`} value={`${totalCpu.toFixed(0)}%`} ratio={totalCpu / 100} detail={info?.load ? `load ${info.load.map((l) => l.toFixed(2)).join(" ")}` : undefined} />
        <Meter icon={MemoryStick} label="Memory" value={bytes(memUsed)} ratio={memTotal ? memUsed / memTotal : undefined} detail={`of ${bytes(memTotal)}`} />
        <Meter
          icon={HardDrive}
          label="Disk (home)"
          value={info?.disk ? `${bytes(info.disk.free)} free` : "–"}
          ratio={info?.disk ? 1 - info.disk.free / info.disk.total : undefined}
          detail={info?.disk ? `of ${bytes(info.disk.total)}` : undefined}
        />
        <Meter icon={Timer} label="Up" value={duration(info?.uptime_s)} detail={info?.kernel ? `Linux ${info.kernel}` : undefined} />
      </div>

      <div className="flex flex-wrap items-center gap-2 border-b hairline px-3 py-2">
        <label className="flex min-w-48 flex-1 items-center gap-2 rounded-md border hairline bg-bg/40 px-2 py-1">
          <Search className="size-3.5 text-muted" />
          <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="Filter by name, command or PID" className="w-full bg-transparent text-sm outline-none" />
        </label>
        <div role="tablist" className="inline-flex rounded-lg bg-fg/[0.06] p-0.5 text-xs">
          {[
            [true, `Mine`],
            [false, `All (${list.processes.length})`],
          ].map(([v, label]) => (
            <button
              key={String(v)}
              type="button"
              role="tab"
              aria-selected={mine === v}
              onClick={() => setMine(v as boolean)}
              className={`rounded-md px-2.5 py-1 ${mine === v ? "bg-raised font-semibold shadow-sm" : "text-muted hover:text-fg"}`}
            >
              {label as string}
            </button>
          ))}
        </div>
        <select value={sort} onChange={(e) => setSort(e.target.value as typeof sort)} className="rounded-md border hairline bg-bg/40 px-2 py-1 text-xs" aria-label="Sort by">
          <option value="cpu">Sort by CPU</option>
          <option value="rss">Sort by memory</option>
          <option value="pid">Sort by PID</option>
        </select>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto">
        <table className="w-full text-sm">
          <thead className="sticky top-0 bg-surface text-xs text-muted">
            <tr className="text-left">
              <th className="w-20 px-3 py-1.5 font-medium">PID</th>
              <th className="px-3 py-1.5 font-medium">Process</th>
              <th className="w-24 px-3 py-1.5 font-medium">User</th>
              <th className="w-16 px-3 py-1.5 text-right font-medium">CPU</th>
              <th className="w-24 px-3 py-1.5 text-right font-medium">Memory</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((p) => (
              <tr
                key={p.pid}
                onClick={() => setSelected(p.pid)}
                className={`cursor-default ${selected === p.pid ? "bg-accent/15" : "hover:bg-fg/[0.04]"}`}
              >
                <td className="px-3 py-1 font-mono text-xs text-muted">{p.pid}</td>
                <td className="max-w-0 px-3 py-1">
                  <p className="truncate font-medium">{p.name}</p>
                  <p className="truncate font-mono text-[11px] text-muted" title={p.cmd}>
                    {p.cmd}
                  </p>
                </td>
                <td className="px-3 py-1 text-muted">{p.user}</td>
                <td className="px-3 py-1 text-right tabular-nums">{p.cpu.toFixed(1)}%</td>
                <td className="px-3 py-1 text-right text-muted tabular-nums">{p.kernel ? "" : bytes(p.rss)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="flex items-center gap-2 border-t hairline px-3 py-2">
        <span className="min-w-0 flex-1 truncate text-xs text-muted">
          {sel ? `${sel.name} · PID ${sel.pid} · ${sel.threads} threads · ${sel.user}` : "Select a process."}
          {sel && !ownSelected ? " · belongs to another user, so FluxOS cannot stop it from here" : ""}
        </span>
        <Button size="sm" variant="subtle" disabled={!ownSelected} busy={busy} onClick={() => void stop("TERM")}>
          Quit
        </Button>
        <Button size="sm" variant="danger" icon={Ban} disabled={!ownSelected} busy={busy} onClick={() => void stop("KILL")}>
          Force quit
        </Button>
      </div>
    </div>
  );
}
