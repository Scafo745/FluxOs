import { Ban, CircleCheck, CircleX, Eye, Inbox, Power, ShieldCheck, TriangleExclamationPoint, Undo2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useShallow } from "zustand/react/shallow";

import { ArrowText, Button, ChangeList, Empty, Section, TierBadge } from "../components/ui";
import { api } from "../lib/api";
import { PLAN_STATUS, TIER_BEHAVIOR, ago, shortHash } from "../lib/format";
import type { HealthCheck, PlanRecord } from "../lib/types";
import { approvePlan, errorText, rejectPlan, restart, undoGeneration } from "../state/actions";
import type { Win } from "../state/shell";
import { useShell } from "../state/shell";
import { selectPending, useSystem } from "../state/system";

function firstLine(s: string): string {
  return s.split("\n")[0] ?? "";
}

export function title(p: PlanRecord): string {
  return p.intent.prompt || firstLine(p.change_summary) || p.id;
}

export function Checks({ checks }: { checks: HealthCheck[] }) {
  return (
    <ul className="grid gap-1 text-sm sm:grid-cols-2">
      {checks.map((c) => (
        <li key={c.id} className="flex items-start gap-2">
          {c.status === "pass" ? (
            <CircleCheck className="mt-0.5 size-4 shrink-0 text-ok" />
          ) : c.status === "warn" ? (
            <TriangleExclamationPoint className="mt-0.5 size-4 shrink-0 text-warn" />
          ) : (
            <CircleX className="mt-0.5 size-4 shrink-0 text-bad" />
          )}
          <span className="min-w-0">
            <span className="font-medium">{c.name}</span> <span className="text-muted">{c.detail}</span>
          </span>
        </li>
      ))}
    </ul>
  );
}

function PlanDetail({ plan }: { plan: PlanRecord }) {
  const whoami = useSystem((s) => s.whoami);
  const [busy, setBusy] = useState<"approve" | "reject" | "preview" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const tier = plan.evaluation.tier;
  const waiting = (plan.status === "proposed" || plan.status === "approved") && plan.decision !== "auto_apply" && plan.decision !== "blocked";
  const canApprove = whoami?.can_approve_tiers.includes(tier) ?? false;
  const warnings = plan.evaluation.issues.filter((i) => i.severity === "warning");
  const errors = plan.evaluation.issues.filter((i) => i.severity === "error");
  const st = PLAN_STATUS[plan.status];
  const engine = plan.engine as { kind?: string; provider?: string; model?: string } | null;
  const gen = plan.result?.generation ?? null;
  const restartPending = useSystem((s) => gen !== null && s.system?.status === "staged" && s.system.generation === gen);

  return (
    <div className="flex h-full flex-col">
      <div className="border-b hairline p-5">
        <div className="flex flex-wrap items-center gap-2 text-xs">
          <TierBadge tier={tier} />
          <span className={`font-semibold ${st.tone}`}>{st.label}</span>
          <span className="text-muted">· {ago(plan.created_at)}</span>
        </div>
        <h2 className="mt-2 text-xl font-bold">
          <ArrowText text={title(plan)} />
        </h2>
        <p className="mt-1 text-sm text-muted">
          Proposed by <span className="text-fg">{plan.proposed_by}</span> via {plan.intent.source ?? "api"}
          {engine?.kind === "llm" ? ` · ${engine.provider}/${engine.model}` : engine?.kind ? ` · ${engine.kind}` : ""}. {TIER_BEHAVIOR[tier]}.
        </p>
        {plan.evaluation.requires_reboot && (
          <p className="mt-3 flex items-center gap-2 rounded-lg bg-warn/10 px-3 py-2 text-sm text-warn">
            <Power className="size-4 shrink-0" /> Takes effect after a restart. The previous version stays in the boot menu.
          </p>
        )}
      </div>

      <div className="min-h-0 flex-1 space-y-6 overflow-y-auto p-5">
        <Section title={`What changes (${plan.evaluation.changes.length})`} right={<span className="text-xs text-muted">computed by fluxd from the real diff</span>}>
          {plan.evaluation.changes.length ? <ChangeList changes={plan.evaluation.changes} /> : <p className="text-sm text-muted">No changes.</p>}
        </Section>

        {errors.length > 0 && (
          <Section title="Why FluxOS refused it">
            <ul className="space-y-1.5 text-sm">
              {errors.map((i, n) => (
                <li key={n} className="flex gap-2 text-bad">
                  <Ban className="mt-0.5 size-4 shrink-0" /> {i.message}
                </li>
              ))}
            </ul>
          </Section>
        )}

        {warnings.length > 0 && (
          <Section title="Warnings">
            <ul className="space-y-1.5 text-sm">
              {warnings.map((i, n) => (
                <li key={n} className="flex gap-2 text-warn">
                  <TriangleExclamationPoint className="mt-0.5 size-4 shrink-0" /> {i.message}
                </li>
              ))}
            </ul>
          </Section>
        )}

        {(plan.ai_summary || plan.ai_rationale) && (
          <Section title="The AI's description">
            <div className="rounded-lg border border-dashed hairline p-3 text-sm">
              {plan.ai_summary && <p>{plan.ai_summary}</p>}
              {plan.ai_rationale && <p className="mt-1 text-muted">{plan.ai_rationale}</p>}
              <p className="mt-2 text-xs text-muted">For context only. Approval is based on the change list above, not on this text.</p>
            </div>
          </Section>
        )}

        {plan.preview && plan.preview.checks.length > 0 && !plan.result && (
          <Section title="Pre-flight checks">
            <Checks checks={plan.preview.checks} />
          </Section>
        )}

        {plan.result && (
          <Section title="Result">
            <p className="mb-2 text-sm">
              <ArrowText text={plan.result.message} />
            </p>
            {plan.result.health.length > 0 && <Checks checks={plan.result.health} />}
          </Section>
        )}

        <Section title="Operations">
          <ul className="space-y-1 font-mono text-xs">
            {plan.operations.map((o, i) => (
              <li key={i} className="truncate rounded bg-fg/[0.04] px-2 py-1" title={JSON.stringify(o.params)}>
                <span className="text-accent">{o.tool}</span> <span className="text-muted">{JSON.stringify(o.params)}</span>
              </li>
            ))}
          </ul>
          <p className="mt-2 text-xs text-muted">
            Plan {plan.id} · hash {shortHash(plan.plan_hash, 12)}
            {plan.approval && ` · approved by ${plan.approval.by} ${ago(plan.approval.at)}`}
          </p>
        </Section>
      </div>

      {error && <p className="border-t hairline px-5 py-2 text-sm text-bad">{error}</p>}
      {waiting && (
        <div className="flex flex-wrap items-center gap-2 border-t hairline p-4">
          <span className="min-w-0 flex-1 text-xs text-muted">
            {canApprove ? "Approving binds to this exact plan; if it changes, FluxOS asks again." : `Your role can approve tiers ${whoami?.can_approve_tiers.join(", ") || "none"}.`}
          </span>
          <Button
            variant="subtle"
            icon={Eye}
            busy={busy === "preview"}
            onClick={async () => {
              setBusy("preview");
              setError(null);
              try {
                await useSystem.getState().previewPlan(plan.id, title(plan), tier);
              } catch (e) {
                setError(errorText(e));
              } finally {
                setBusy(null);
              }
            }}
          >
            Preview
          </Button>
          <Button
            variant="ghost"
            busy={busy === "reject"}
            onClick={async () => {
              setBusy("reject");
              await rejectPlan(plan.id);
              setBusy(null);
            }}
          >
            Reject
          </Button>
          <Button
            variant="primary"
            icon={ShieldCheck}
            disabled={!canApprove}
            busy={busy === "approve"}
            onClick={async () => {
              setBusy("approve");
              await approvePlan(plan);
              setBusy(null);
            }}
          >
            Approve & apply
          </Button>
        </div>
      )}
      {(plan.status === "applied" || plan.status === "staged") && gen !== null && (
        <div className="flex justify-end gap-2 border-t hairline p-4">
          {restartPending && (
            <Button variant="primary" icon={Power} onClick={() => void restart()}>
              Restart now
            </Button>
          )}
          <Button icon={Undo2} onClick={() => void undoGeneration(gen)}>
            Undo
          </Button>
        </div>
      )}
    </div>
  );
}

export function Approvals({ win }: { win: Win }) {
  const plans = useSystem((s) => s.plans);
  const pending = useSystem(useShallow(selectPending));
  const requested = win.props.planId as string | undefined;
  const [selectedId, setSelectedId] = useState<string | undefined>(requested);
  const [extra, setExtra] = useState<PlanRecord | null>(null);

  useEffect(() => {
    if (requested) setSelectedId(requested);
  }, [requested]);

  const decided = useMemo(() => plans.filter((p) => !pending.includes(p) && p.status !== "no_op").slice(0, 30), [plans, pending]);
  const id = selectedId ?? pending[0]?.id ?? decided[0]?.id;
  const selected = plans.find((p) => p.id === id) ?? (extra?.id === id ? extra : undefined);

  useEffect(() => {
    if (id && !plans.some((p) => p.id === id) && extra?.id !== id) {
      api
        .plan(id)
        .then(setExtra)
        .catch(() => setExtra(null));
    }
  }, [id, plans, extra]);

  const Item = ({ p }: { p: PlanRecord }) => {
    const st = PLAN_STATUS[p.status];
    return (
      <button
        type="button"
        onClick={() => setSelectedId(p.id)}
        className={`w-full rounded-lg px-3 py-2.5 text-left transition-colors ${p.id === id ? "bg-accent/15 ring-1 ring-accent/40" : "hover:bg-fg/[0.05]"}`}
      >
        <div className="flex items-center gap-2">
          <TierBadge tier={p.evaluation.tier} label={false} />
          <span className={`text-xs font-semibold ${st.tone}`}>{st.label}</span>
          <span className="ml-auto text-xs text-muted">{ago(p.created_at)}</span>
        </div>
        <p className="mt-1 line-clamp-2 text-sm font-medium">
          <ArrowText text={title(p)} />
        </p>
        <p className="text-xs text-muted">
          {p.evaluation.changes.length} changes · {p.proposed_by}
        </p>
      </button>
    );
  };

  return (
    <div className="flex h-full">
      <aside className="flex w-72 shrink-0 flex-col overflow-y-auto border-r hairline p-2">
        <p className="px-2 pt-2 pb-1 text-xs font-semibold tracking-[0.12em] text-muted uppercase">Waiting for you ({pending.length})</p>
        {pending.length === 0 && <p className="px-2 pb-2 text-sm text-muted">Nothing needs approval.</p>}
        {pending.map((p) => (
          <Item key={p.id} p={p} />
        ))}
        <p className="mt-3 px-2 pt-2 pb-1 text-xs font-semibold tracking-[0.12em] text-muted uppercase">Recent</p>
        {decided.map((p) => (
          <Item key={p.id} p={p} />
        ))}
      </aside>
      <div className="min-w-0 flex-1">
        {selected ? (
          <PlanDetail plan={selected} />
        ) : (
          <Empty icon={Inbox} title="No plans yet">
            Ask Flux for a change. Anything above tier 1 lands here for review.
            <div className="mt-3">
              <Button size="sm" variant="primary" onClick={() => useShell.getState().show("assistant")}>
                Ask Flux
              </Button>
            </div>
          </Empty>
        )}
      </div>
    </div>
  );
}
