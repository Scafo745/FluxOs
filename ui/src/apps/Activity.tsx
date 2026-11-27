import { ChevronRight, RefreshCw, ShieldCheck, ShieldX } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import { ArrowText, Button, Spinner, Tabs } from "../components/ui";
import { api } from "../lib/api";
import { dateTime, shortHash } from "../lib/format";
import type { AuditEntry, VerifyReport } from "../lib/types";
import { errorText } from "../state/actions";
import { useSystem } from "../state/system";

const FILTERS = ["all", "plan", "apply", "generation", "session", "system", "profile"] as const;

function kindTone(kind: string): string {
  if (kind.endsWith("blocked") || kind.endsWith("rolled_back") || kind.endsWith("failed") || kind.startsWith("pairing")) return "text-bad bg-bad/12";
  if (kind.startsWith("apply.committed") || kind.startsWith("generation")) return "text-ok bg-ok/12";
  if (kind.startsWith("plan.approved") || kind.startsWith("session")) return "text-warn bg-warn/12";
  if (kind.startsWith("plan")) return "text-accent bg-accent/12";
  return "text-muted bg-fg/[0.06]";
}

function Entry({ e }: { e: AuditEntry }) {
  const [open, setOpen] = useState(false);
  return (
    <li className="border-b hairline last:border-b-0">
      <button type="button" onClick={() => setOpen((v) => !v)} className="grid w-full grid-cols-[16px_44px_110px_160px_1fr] items-center gap-2 px-3 py-2 text-left text-sm hover:bg-fg/[0.04]">
        <ChevronRight className={`size-3.5 text-muted transition-transform ${open ? "rotate-90" : ""}`} />
        <span className="font-mono text-xs text-muted">#{e.seq}</span>
        <span className="text-xs text-muted tabular-nums">{dateTime(e.ts).replace(/^[A-Za-z]{3} \d+, /, "")}</span>
        <span className={`truncate rounded px-1.5 py-0.5 font-mono text-[11px] ${kindTone(e.kind)}`}>{e.kind}</span>
        <span className="min-w-0 truncate">
          <span className="text-muted">{e.actor}</span> · <ArrowText text={e.summary} />
        </span>
      </button>
      {open && (
        <div className="space-y-2 bg-bg/30 px-10 py-3 text-xs">
          <pre className="max-h-64 overflow-auto rounded-md bg-fg/[0.04] p-2 font-mono whitespace-pre-wrap">
            <ArrowText text={JSON.stringify(e.details, null, 2)} />
          </pre>
          <p className="font-mono text-muted">
            hash {shortHash(e.hash, 20)} · prev {shortHash(e.prev_hash, 20)}
          </p>
        </div>
      )}
    </li>
  );
}

function AuditLog() {
  const [filter, setFilter] = useState<(typeof FILTERS)[number]>("all");
  const [entries, setEntries] = useState<AuditEntry[]>([]);
  const [verify, setVerify] = useState<VerifyReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const head = useSystem((s) => s.system?.audit_head[0]);

  const load = useCallback(
    async (before?: number) => {
      setBusy(true);
      try {
        const r = await api.audit({ before, limit: 60, kind: filter === "all" ? undefined : `${filter}.` });
        setEntries((old) => (before ? [...old, ...r.entries] : r.entries));
        setError(null);
      } catch (e) {
        setError(errorText(e));
      } finally {
        setBusy(false);
      }
    },
    [filter],
  );

  const check = useCallback(() => {
    api
      .auditVerify()
      .then(setVerify)
      .catch((e) => setError(errorText(e)));
  }, []);

  useEffect(() => {
    void load();
  }, [load, head]);
  useEffect(() => {
    check();
  }, [check, head]);

  return (
    <div className="flex h-full flex-col">
      <div className="flex flex-wrap items-center gap-3 border-b hairline px-4 py-3">
        {verify ? (
          verify.ok ? (
            <span className="flex items-center gap-2 rounded-full bg-ok/12 px-3 py-1 text-sm font-medium text-ok">
              <ShieldCheck className="size-4" /> Chain verified · {verify.entries} entries · head {shortHash(verify.head, 10)}
            </span>
          ) : (
            <span className="flex items-center gap-2 rounded-full bg-bad/12 px-3 py-1 text-sm font-medium text-bad">
              <ShieldX className="size-4" /> Chain broken at #{verify.broken_at}: {verify.error}
            </span>
          )
        ) : (
          <Spinner />
        )}
        <Button size="sm" variant="ghost" icon={RefreshCw} onClick={check}>
          Verify again
        </Button>
        <div className="flex-1" />
        <div className="flex flex-wrap gap-1">
          {FILTERS.map((f) => (
            <button
              key={f}
              type="button"
              onClick={() => setFilter(f)}
              className={`rounded-md px-2 py-1 text-xs capitalize ${filter === f ? "bg-accent text-on-accent" : "text-muted hover:bg-fg/[0.07] hover:text-fg"}`}
            >
              {f}
            </button>
          ))}
        </div>
      </div>
      {error && <p className="px-4 py-2 text-sm text-bad">{error}</p>}
      <ul className="min-h-0 flex-1 overflow-y-auto">
        {entries.map((e) => (
          <Entry key={e.seq} e={e} />
        ))}
        {entries.length > 0 && (entries[entries.length - 1]?.seq ?? 0) > 1 && (
          <li className="p-3 text-center">
            <Button size="sm" variant="ghost" busy={busy} onClick={() => void load(entries[entries.length - 1]?.seq)}>
              Load older
            </Button>
          </li>
        )}
      </ul>
    </div>
  );
}

function LiveEvents() {
  const events = useSystem((s) => s.events);
  return (
    <ul className="h-full overflow-y-auto font-mono text-xs">
      {events.length === 0 && <li className="p-4 text-muted">Waiting for events. Make a change to see them stream in.</li>}
      {[...events].reverse().map((ev) => (
        <li key={ev.seq} className="grid grid-cols-[56px_140px_1fr] gap-2 border-b hairline px-4 py-1.5">
          <span className="text-muted">#{ev.seq}</span>
          <span className={`truncate ${kindTone(ev.kind)} rounded px-1`}>{ev.kind}</span>
          <span className="truncate text-muted" title={JSON.stringify(ev.data)}>
            <ArrowText text={JSON.stringify(ev.data)} />
          </span>
        </li>
      ))}
    </ul>
  );
}

export function Activity() {
  const [tab, setTab] = useState<"audit" | "live">("audit");
  return (
    <div className="flex h-full flex-col">
      <div className="border-b hairline px-4 py-3">
        <Tabs
          value={tab}
          onChange={setTab}
          options={[
            { id: "audit", label: "Audit log" },
            { id: "live", label: "Live events" },
          ]}
        />
      </div>
      <div className="min-h-0 flex-1">{tab === "audit" ? <AuditLog /> : <LiveEvents />}</div>
    </div>
  );
}
