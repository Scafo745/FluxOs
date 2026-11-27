import type { LucideIcon } from "lucide-react";
import { Ban, FingerprintPattern, KeyRound, Lock, MonitorSmartphone, Network, ScrollText, ShieldCheck, ShieldX, Usb } from "lucide-react";
import { useEffect, useState } from "react";

import { ArrowText, Button, Section } from "../components/ui";
import { api } from "../lib/api";
import { ago } from "../lib/format";
import type { AuditEntry, VerifyReport } from "../lib/types";
import { useAssistant } from "../state/assistant";
import { useShell } from "../state/shell";
import { selectSpec, useSystem } from "../state/system";

const LOCKDOWN =
  "Lock this machine down: block USB storage, close every open port and lock the screen after 5 minutes";

function Check({ icon: Icon, label, value, ok, note }: { icon: LucideIcon; label: string; value: string; ok: boolean | null; note?: string }) {
  return (
    <div className="card flex items-start gap-3 p-3">
      <span className={`grid size-8 shrink-0 place-items-center rounded-lg ${ok === null ? "bg-fg/[0.07] text-muted" : ok ? "bg-ok/15 text-ok" : "bg-warn/15 text-warn"}`}>
        <Icon className="size-4" />
      </span>
      <div className="min-w-0">
        <p className="text-xs text-muted">{label}</p>
        <p className="font-semibold">{value}</p>
        {note && <p className="mt-0.5 text-xs text-muted">{note}</p>}
      </div>
    </div>
  );
}

/**
 * The machine's security posture, from the live spec and fluxd's audit log.
 * Everything here is enforced by fluxd; the AI can propose changes but tier
 * 2+ security changes wait for a person, and some are never allowed.
 */
export function Security() {
  const spec = useSystem(selectSpec);
  const whoami = useSystem((s) => s.whoami);
  const head = useSystem((s) => s.system?.audit_head[0]);
  const [blocked, setBlocked] = useState<AuditEntry[]>([]);
  const [verify, setVerify] = useState<VerifyReport | null>(null);
  const [screens, setScreens] = useState<number | null>(null);

  useEffect(() => {
    api
      .audit({ kind: "plan.blocked", limit: 12 })
      .then((r) => setBlocked(r.entries))
      .catch(() => setBlocked([]));
    api
      .auditVerify()
      .then(setVerify)
      .catch(() => setVerify(null));
    api
      .sessions()
      .then((r) => setScreens(r.sessions.length))
      .catch(() => setScreens(null));
  }, [head]);

  if (!spec) return null;
  const sec = spec.security;
  const tcp = sec.firewall.allowed_tcp_ports;
  const udp = sec.firewall.allowed_udp_ports;
  const open = tcp.length + udp.length;
  const hardened = spec.kernel.variant === "hardened";
  const lockedDown = sec.usb_storage === "block" && open === 0 && sec.screen_lock.enabled;
  const posture = spec.session.mode === "exam" ? "Exam lock-down" : lockedDown ? "Locked down" : "Protected";

  return (
    <div className="h-full space-y-6 overflow-y-auto p-5">
      <div className="flex flex-wrap items-center gap-4 rounded-xl border hairline bg-ok/[0.06] p-4">
        <span className="grid size-12 place-items-center rounded-2xl bg-ok/15 text-ok">
          <ShieldCheck className="size-7" />
        </span>
        <div className="min-w-0 flex-1">
          <p className="text-xs font-semibold tracking-[0.12em] text-muted uppercase">Security posture</p>
          <p className="text-xl font-bold">{posture}</p>
          <p className="text-sm text-muted">
            Firewall and audit are always on: fluxd refuses plans that turn them off. {whoami?.mode === "fleet" ? "Fleet mode: an administrator approves system changes." : "Personal mode: you approve system changes."}
          </p>
        </div>
        <Button
          variant="primary"
          icon={Lock}
          onClick={() => {
            useShell.getState().show("assistant");
            void useAssistant.getState().ask(LOCKDOWN);
          }}
        >
          Lock down
        </Button>
      </div>

      <Section title="Enforced settings">
        <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
          <Check icon={Network} label="Firewall" value={open ? `On · ${open} open port${open === 1 ? "" : "s"}` : "On · no inbound ports"} ok={true} note={open ? `TCP ${tcp.join(", ") || "none"} · UDP ${udp.join(", ") || "none"}` : undefined} />
          <Check icon={ShieldCheck} label="Mandatory access control" value={sec.mac === "apparmor" ? "AppArmor" : "Off"} ok={sec.mac === "apparmor"} />
          <Check icon={Usb} label="USB storage" value={sec.usb_storage === "block" ? "Blocked" : "Allowed"} ok={sec.usb_storage === "block" ? true : null} />
          <Check
            icon={Lock}
            label="Screen lock"
            value={sec.screen_lock.enabled ? `After ${sec.screen_lock.idle_minutes} min` : "Off"}
            ok={sec.screen_lock.enabled ? (spec.desktop.environment === "fluxos" ? null : true) : false}
            note={spec.desktop.environment === "fluxos" ? "Applied by Plasma and GNOME; the FluxOS shell does not lock yet." : undefined}
          />
          <Check icon={ScrollText} label="Kernel audit" value={sec.audit ? "On" : "Off"} ok={sec.audit} />
          <Check icon={FingerprintPattern} label="Kernel" value={hardened ? "Hardened" : spec.kernel.variant} ok={hardened ? true : null} note={hardened ? "KSPP boot parameters and sysctls" : undefined} />
        </div>
      </Section>

      <Section
        title="Audit chain"
        right={
          <Button size="sm" variant="ghost" onClick={() => useShell.getState().open("activity")}>
            Open Activity
          </Button>
        }
      >
        {verify ? (
          <p className={`flex items-center gap-2 text-sm ${verify.ok ? "text-ok" : "text-bad"}`}>
            {verify.ok ? <ShieldCheck className="size-4" /> : <ShieldX className="size-4" />}
            {verify.ok ? `All ${verify.entries} entries verified: no entry was changed or removed.` : `Broken at entry #${verify.broken_at}: ${verify.error}`}
          </p>
        ) : (
          <p className="text-sm text-muted">Checking…</p>
        )}
      </Section>

      <Section title={`Refused by policy (${blocked.length})`}>
        {blocked.length === 0 ? (
          <p className="text-sm text-muted">Nothing refused yet. Try asking Flux to “turn off the firewall”.</p>
        ) : (
          <ul className="divide-y divide-line/40 text-sm">
            {blocked.map((e) => (
              <li key={e.seq} className="flex items-start gap-2 py-2">
                <Ban className="mt-0.5 size-4 shrink-0 text-bad" />
                <span className="min-w-0 flex-1">
                  <ArrowText text={e.summary} />
                  <span className="block text-xs text-muted">
                    {e.actor} · {ago(e.ts)}
                  </span>
                </span>
              </li>
            ))}
          </ul>
        )}
      </Section>

      <Section title="Access">
        <div className="grid gap-2 sm:grid-cols-2">
          <Check icon={MonitorSmartphone} label="Paired screens" value={screens === null ? "–" : String(screens)} ok={null} note="Revoke screens in System › Screens." />
          <Check icon={KeyRound} label="This screen" value={whoami ? `${whoami.role}${whoami.session ? " (paired)" : ""}` : "–"} ok={null} note={whoami ? `Approves tiers ${whoami.can_approve_tiers.join(", ") || "none"}` : undefined} />
        </div>
      </Section>
    </div>
  );
}
