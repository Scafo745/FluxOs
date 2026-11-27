import type { LucideIcon } from "lucide-react";
import {
  Accessibility,
  ArrowUp,
  Ban,
  BookOpen,
  BrainCircuit,
  Check,
  ChevronDown,
  ChevronRight,
  CircleCheck,
  CircleQuestionMark,
  CircleX,
  CornerDownRight,
  Cpu,
  Eye,
  FileCheck,
  Gamepad2,
  GitCommitVertical,
  GraduationCap,
  HeartPulse,
  MessageSquare,
  Network,
  Palette,
  Power,
  Route,
  ShieldCheck,
  ShieldOff,
  Sparkles,
  TriangleExclamationPoint,
  Undo2,
  Wrench,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useShallow } from "zustand/react/shallow";

import { FluxMark } from "../components/FluxMark";
import { ArrowText, Button, ChangeList, Chip, Kbd, Spinner, TierBadge } from "../components/ui";
import { api } from "../lib/api";
import { TIER_LABEL } from "../lib/format";
import type { IntentResult, IntentStatus, TraceStep } from "../lib/types";
import { errorText, reportPlan, restart, undoGeneration } from "../state/actions";
import { type Turn, useAssistant } from "../state/assistant";
import { useShell } from "../state/shell";
import { useSystem } from "../state/system";

const EXAMPLES: { text: string; icon: LucideIcon }[] = [
  { text: "Make it feel like macOS", icon: Sparkles },
  { text: "Prioritize gaming above everything", icon: Gamepad2 },
  { text: "Set this up for a telecom operations desk", icon: Network },
  { text: "Neon green and pink theme", icon: Palette },
  { text: "Bigger text and reduce animations", icon: Accessibility },
  { text: "Set this up for my school", icon: GraduationCap },
  { text: "Turn off the firewall", icon: ShieldOff },
  { text: "What changed recently?", icon: GitCommitVertical },
];

const STAGE: Record<string, { label: string; icon: LucideIcon }> = {
  intake: { label: "Request", icon: MessageSquare },
  context: { label: "Read the system", icon: Cpu },
  retrieve: { label: "Knowledge", icon: BookOpen },
  route: { label: "Routing", icon: Route },
  provider: { label: "Model call", icon: BrainCircuit },
  generate: { label: "Drafted plan", icon: Sparkles },
  clarify: { label: "Question", icon: CircleQuestionMark },
  validate: { label: "Policy check", icon: ShieldCheck },
  repair: { label: "Self-repair", icon: Wrench },
  propose: { label: "Plan recorded", icon: FileCheck },
  verify: { label: "Health checks", icon: HeartPulse },
  rollback: { label: "Undo", icon: Undo2 },
  done: { label: "Result", icon: CircleCheck },
};

const HEAD: Record<IntentStatus, { icon: LucideIcon; tone: string; title: (r: IntentResult) => string }> = {
  applied: { icon: CircleCheck, tone: "text-ok", title: (r) => `Applied · generation ${r.result?.generation ?? ""} is live` },
  staged: { icon: Power, tone: "text-warn", title: (r) => `Installed as generation ${r.result?.generation ?? ""} · restart to finish` },
  awaiting_approval: { icon: ShieldCheck, tone: "text-accent", title: (r) => `Needs your approval · tier ${r.plan?.tier} ${TIER_LABEL[r.plan?.tier ?? 0].toLowerCase()} risk` },
  needs_clarification: { icon: CircleQuestionMark, tone: "text-accent", title: () => "One question first" },
  answered: { icon: MessageSquare, tone: "text-fg", title: () => "Answer" },
  blocked: { icon: Ban, tone: "text-bad", title: () => "Refused by FluxOS policy" },
  invalid: { icon: TriangleExclamationPoint, tone: "text-bad", title: () => "No valid plan" },
  no_op: { icon: Check, tone: "text-muted", title: () => "Already set" },
  rolled_back: { icon: Undo2, tone: "text-bad", title: () => "Rolled back automatically" },
  failed: { icon: CircleX, tone: "text-bad", title: () => "Apply failed" },
  error: { icon: CircleX, tone: "text-bad", title: () => "Could not do that" },
};

function stepTone(s: TraceStep): string {
  if (s.stage === "validate") return s.detail.startsWith("rejected") ? "text-bad" : "text-ok";
  if (s.stage === "provider") return s.detail.includes("failed") ? "text-warn" : "text-accent";
  if (s.stage === "done") return /^(applied|staged|answered|awaiting)/.test(s.detail) ? "text-ok" : "text-muted";
  if (s.stage === "verify") return s.detail.startsWith("applied") || s.detail.startsWith("staged") ? "text-ok" : "text-warn";
  return "text-muted";
}

function Steps({ steps, running, collapsed, onToggle }: { steps: TraceStep[]; running: boolean; collapsed: boolean; onToggle: () => void }) {
  const total = steps[steps.length - 1]?.ms ?? 0;
  const route = steps.find((s) => s.stage === "route")?.detail ?? "";
  if (collapsed) {
    return (
      <button type="button" onClick={onToggle} className="mt-2 flex items-center gap-1.5 text-xs text-muted hover:text-fg">
        <ChevronRight className="size-3.5" />
        {steps.length} steps · {total} ms · {route.split("(")[0]?.trim()}
      </button>
    );
  }
  return (
    <div className="mt-3">
      {!running && (
        <button type="button" onClick={onToggle} className="mb-1 flex items-center gap-1.5 text-xs text-muted hover:text-fg">
          <ChevronDown className="size-3.5" /> Pipeline
        </button>
      )}
      <ol className="relative ml-2 border-l hairline pl-4" aria-label="AI pipeline steps">
        {steps.map((s, i) => {
          const meta = STAGE[s.stage] ?? { label: s.stage, icon: Sparkles };
          const Icon = meta.icon;
          return (
            <li key={i} className="rise-in relative grid grid-cols-[110px_1fr_auto] items-baseline gap-2 py-[3px] text-xs">
              <span className={`absolute top-[5px] -left-[23px] grid size-3.5 place-items-center rounded-full bg-surface ${stepTone(s)}`}>
                <Icon className="size-3" />
              </span>
              <span className="font-semibold text-fg/90">{meta.label}</span>
              <span className="truncate text-muted" title={s.detail}>
                <ArrowText text={s.detail} />
              </span>
              <span className="font-mono text-muted/70 tabular-nums">{s.ms} ms</span>
            </li>
          );
        })}
        {running && (
          <li className="relative flex items-center gap-2 py-[3px] text-xs text-muted">
            <span className="absolute -left-[23px] grid size-3.5 place-items-center rounded-full bg-surface text-accent">
              <Spinner className="size-3" />
            </span>
            Working…
          </li>
        )}
      </ol>
    </div>
  );
}

function EngineBadge({ r }: { r: IntentResult }) {
  const e = r.engine;
  const text =
    e.kind === "heuristic"
      ? `local rules${e.coverage !== undefined ? ` · ${Math.round(e.coverage * 100)}% understood` : ""}`
      : e.kind === "llm"
        ? `${e.provider} · ${e.model}`
        : e.kind === "cache"
          ? `cached plan · ${e.model ?? ""}`
          : e.kind;
  return (
    <Chip title="Which engine produced this plan">
      {e.kind === "llm" ? <BrainCircuit className="size-3" /> : <Cpu className="size-3" />}
      {text}
    </Chip>
  );
}

function Clarify({ turn, r }: { turn: Turn; r: IntentResult }) {
  const [free, setFree] = useState("");
  const ask = useAssistant((s) => s.ask);
  return (
    <div className="mt-3 space-y-3">
      {(r.questions ?? []).map((q) => (
        <div key={q.id} className="rounded-lg bg-fg/[0.05] p-3">
          <p className="font-medium">{q.question}</p>
          {q.why && <p className="mt-0.5 text-xs text-muted">{q.why}</p>}
          <div className="mt-2 flex flex-wrap gap-2">
            {(q.options ?? []).map((o) => (
              <Button key={o} size="sm" variant="subtle" onClick={() => void ask(turn.prompt, { ...turn.answers, [q.id]: o })}>
                {o}
              </Button>
            ))}
          </div>
          <form
            className="mt-2 flex gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              if (free.trim()) void ask(turn.prompt, { ...turn.answers, [q.id]: free.trim() });
            }}
          >
            <input
              value={free}
              onChange={(e) => setFree(e.target.value)}
              placeholder="Or type an answer"
              className="h-7 flex-1 rounded-md border hairline bg-bg/50 px-2 text-sm outline-none focus:border-accent"
            />
            <Button size="sm" type="submit" disabled={!free.trim()}>
              Answer
            </Button>
          </form>
        </div>
      ))}
    </div>
  );
}

function ResultCard({ turn, r }: { turn: Turn; r: IntentResult }) {
  const head = HEAD[r.status];
  const Icon = head.icon;
  const whoami = useSystem((s) => s.whoami);
  const setFollowUp = useAssistant((s) => s.setFollowUp);
  const lines = r.message.split("\n");
  const notes = lines.filter((l) => l.startsWith("Note:"));
  const plan = r.plan;
  const changes = plan?.changes ?? [];
  const fu = turn.followUp;
  // fluxd's record of the plan, kept current by the event stream. It can be
  // approved or rejected elsewhere (preview banner, Approvals, another screen).
  const record = useSystem((s) => (plan ? s.plans.find((p) => p.id === plan.id) : undefined));
  const decided = fu?.plan ?? (r.status === "awaiting_approval" && record && record.status !== "proposed" ? record : undefined);
  const status = decided?.status ?? null;
  const canApprove = plan ? (whoami?.can_approve_tiers ?? []).includes(plan.tier) : false;

  const description =
    r.status === "answered"
      ? (r.answer ?? r.message)
      : r.status === "applied" || r.status === "staged"
        ? (r.ai_summary ?? lines[0])
        : lines[0];

  async function approve() {
    if (!plan) return;
    setFollowUp(turn.id, { state: "working" });
    try {
      const result = await api.approve(plan.id, plan.plan_hash);
      setFollowUp(turn.id, { state: "done", plan: result });
      reportPlan(result, true);
      void useSystem.getState().refreshPlans();
    } catch (e) {
      setFollowUp(turn.id, { state: "error", message: errorText(e) });
    }
  }

  async function reject() {
    if (!plan) return;
    setFollowUp(turn.id, { state: "working" });
    try {
      const result = await api.reject(plan.id, "Rejected in the assistant");
      setFollowUp(turn.id, { state: "done", plan: result });
      void useSystem.getState().refreshPlans();
    } catch (e) {
      setFollowUp(turn.id, { state: "error", message: errorText(e) });
    }
  }

  const gen = decided?.result?.generation ?? r.result?.generation ?? null;
  const live = status === "applied" || status === "staged" || r.status === "applied" || r.status === "staged";
  const elsewhere = decided !== undefined && fu?.plan === undefined;
  // Plan records stay "staged" after the restart; only offer one while this generation still waits for it.
  const restartPending = useSystem((s) => gen !== null && s.system?.status === "staged" && s.system.generation === gen);

  return (
    <div className="rise-in mt-3 rounded-lg border hairline bg-bg/35 p-3.5">
      <div className="flex flex-wrap items-center gap-2">
        <Icon className={`size-4.5 ${head.tone}`} />
        <h3 className={`font-semibold ${head.tone}`}>{head.title(r)}</h3>
        <div className="flex-1" />
        <EngineBadge r={r} />
      </div>
      {description && (
        <p className={`mt-1.5 text-sm ${r.status === "answered" ? "whitespace-pre-wrap text-fg" : "text-muted"}`}>
          <ArrowText text={description} />
        </p>
      )}
      {r.rationale && r.engine.kind === "llm" && <p className="mt-1 text-xs text-muted">Why: {r.rationale}</p>}

      {changes.length > 0 && (r.status === "applied" || r.status === "staged" || r.status === "awaiting_approval") && (
        <div className="mt-2.5 rounded-md bg-fg/[0.03] px-2.5">
          <ChangeList changes={changes} limit={6} />
        </div>
      )}
      {(r.status === "blocked" || r.status === "invalid") && plan && (
        <ul className="mt-2 space-y-1 text-sm">
          {plan.issues
            .filter((i) => i.severity === "error")
            .map((i, idx) => (
              <li key={idx} className="flex gap-2 text-bad">
                <Ban className="mt-0.5 size-3.5 shrink-0" /> {i.message}
              </li>
            ))}
        </ul>
      )}
      {notes.map((n) => (
        <p key={n} className="mt-2 rounded-md bg-warn/10 px-2.5 py-1.5 text-xs text-warn">
          <ArrowText text={n.replace(/^Note:\s*/, "")} />
        </p>
      ))}
      {r.result?.health && r.result.health.total > 0 && (
        <p className="mt-2 flex items-center gap-1.5 text-xs text-muted">
          <HeartPulse className="size-3.5 text-ok" /> {r.result.health.passed}/{r.result.health.total} health checks passed
        </p>
      )}

      {r.status === "needs_clarification" && <Clarify turn={turn} r={r} />}

      {decided && (
        <p
          className={`mt-3 text-sm font-medium ${
            status === "applied" ? "text-ok" : status === "staged" || status === "approved" ? "text-warn" : status === "rejected" ? "text-muted" : "text-bad"
          }`}
        >
          {status === "approved" && "Approved · applying…"}
          {status === "applied" && `Approved and applied · generation ${gen} is live.`}
          {status === "staged" && (restartPending ? `Approved · generation ${gen} starts after a restart.` : `Approved · generation ${gen} was installed for the next boot.`)}
          {status === "rejected" && "Rejected. Nothing changed."}
          {(status === "rolled_back" || status === "failed") && (
            <ArrowText text={decided.result?.message ?? (status === "failed" ? "Apply failed." : "Rolled back.")} />
          )}
          {elsewhere && decided.approval && status !== "rejected" && <span className="font-normal text-muted"> By {decided.approval.by}.</span>}
        </p>
      )}
      {fu?.state === "error" && <p className="mt-3 text-sm text-bad">{fu.message}</p>}

      <div className="mt-3 flex flex-wrap items-center gap-2">
        {r.status === "awaiting_approval" && plan && !decided && (
          <>
            <Button size="sm" variant="primary" icon={ShieldCheck} busy={fu?.state === "working"} disabled={!canApprove} onClick={() => void approve()}>
              Approve & apply
            </Button>
            <Button size="sm" variant="subtle" icon={Eye} onClick={() => void useSystem.getState().previewPlan(plan.id, turn.prompt, plan.tier)}>
              Preview
            </Button>
            <Button size="sm" variant="ghost" onClick={() => void reject()}>
              Reject
            </Button>
            <Button size="sm" variant="ghost" onClick={() => useShell.getState().open("approvals", { planId: plan.id })}>
              Details
            </Button>
            <TierBadge tier={plan.tier} />
            {!canApprove && <span className="text-xs text-muted">An administrator approves tier {plan.tier} changes in fleet mode.</span>}
          </>
        )}
        {live && gen !== null && (
          <>
            <Button size="sm" variant="subtle" icon={Undo2} onClick={() => void undoGeneration(gen)}>
              Undo
            </Button>
            {restartPending && (
              <Button size="sm" variant="primary" icon={Power} onClick={() => void restart()}>
                Restart now
              </Button>
            )}
            <Button size="sm" variant="ghost" onClick={() => useShell.getState().open("timeline")}>
              Timeline
            </Button>
          </>
        )}
      </div>
    </div>
  );
}

function TurnView({ turn }: { turn: Turn }) {
  const [open, setOpen] = useState(true);
  const done = !turn.running && (turn.result !== null || turn.error !== null);
  useEffect(() => {
    if (!done) return;
    const t = setTimeout(() => setOpen(false), 2400);
    return () => clearTimeout(t);
  }, [done]);
  const answers = Object.values(turn.answers);
  return (
    <article className="border-b hairline px-5 py-4 last:border-b-0">
      <div className="flex flex-wrap items-center gap-2">
        <p className="text-[15px] font-semibold">{turn.prompt}</p>
        {answers.map((a) => (
          <Chip key={a}>
            <CornerDownRight className="size-3 text-muted" /> {a}
          </Chip>
        ))}
      </div>
      {turn.steps.length > 0 && <Steps steps={turn.steps} running={turn.running} collapsed={done && !open} onToggle={() => setOpen((v) => !v)} />}
      {turn.running && turn.steps.length === 0 && (
        <p className="mt-3 flex items-center gap-2 text-sm text-muted">
          <Spinner /> Reaching the FluxOS AI…
        </p>
      )}
      {turn.error && (
        <p className="mt-3 flex items-center gap-2 rounded-lg bg-bad/10 px-3 py-2 text-sm text-bad">
          <CircleX className="size-4" /> {turn.error}
        </p>
      )}
      {turn.result && <ResultCard turn={turn} r={turn.result} />}
    </article>
  );
}

function Welcome({ onPick }: { onPick: (t: string) => void }) {
  return (
    <div className="px-5 py-5">
      <p className="font-display text-xl font-bold">Describe the computer you need.</p>
      <p className="mt-1 text-sm text-muted">
        Looks and apps apply right away, with a snapshot you can undo. System changes wait for your approval. Some things are never allowed.
      </p>
      <div className="mt-4 grid grid-cols-1 gap-2 sm:grid-cols-2">
        {EXAMPLES.map(({ text, icon: Icon }) => (
          <button
            key={text}
            type="button"
            onClick={() => onPick(text)}
            className="flex items-center gap-2.5 rounded-lg border hairline bg-fg/[0.03] px-3 py-2.5 text-left text-sm transition-colors hover:border-accent/60 hover:bg-accent/10"
          >
            <Icon className="size-4 shrink-0 text-accent" />
            {text}
          </button>
        ))}
      </div>
    </div>
  );
}

function EngineLine() {
  const agent = useSystem((s) => s.agent);
  if (agent === "offline") return <span className="text-bad">FluxOS AI offline</span>;
  if (!agent) return <span>Connecting…</span>;
  const live = agent.providers.filter((p) => p.circuit === "closed");
  return (
    <span className="flex items-center gap-1.5">
      <Cpu className="size-3.5" /> Local rules{live.length ? ` + ${live.map((p) => p.model).join(", ")}` : " only"} · checked by fluxd policy
    </span>
  );
}

export function Assistant() {
  const open = useShell((s) => s.overlay === "assistant");
  const { turns, draft, history } = useAssistant(useShallow((s) => ({ turns: s.turns, draft: s.draft, history: s.history })));
  const running = turns.some((t) => t.running);
  const input = useRef<HTMLInputElement>(null);
  const scroller = useRef<HTMLDivElement>(null);
  const [hint, setHint] = useState(0);
  const [recall, setRecall] = useState<number | null>(null);

  useEffect(() => {
    if (open) setTimeout(() => input.current?.focus(), 30);
  }, [open]);
  useEffect(() => {
    const el = scroller.current;
    if (el) el.scrollTo({ top: el.scrollHeight, behavior: "smooth" });
  }, [turns]);
  useEffect(() => {
    const t = setInterval(() => setHint((i) => (i + 1) % EXAMPLES.length), 3200);
    return () => clearInterval(t);
  }, []);

  if (!open) return null;
  const ask = (text: string) => {
    setRecall(null);
    void useAssistant.getState().ask(text);
  };

  return (
    <div
      className="fixed inset-0 z-[58] flex justify-center px-4 fade-in"
      style={{ background: "color-mix(in oklab, var(--flux-color-background) 38%, transparent)" }}
      onMouseDown={(e) => e.target === e.currentTarget && useShell.getState().show(null)}
    >
      <section role="dialog" aria-label="Ask Flux" className="glass glow-ring win-enter mt-[8vh] flex h-fit max-h-[82vh] w-[min(780px,100%)] flex-col rounded-xl shadow-raised">
        <form
          className="flex items-center gap-3 px-4 py-3.5"
          onSubmit={(e) => {
            e.preventDefault();
            if (draft.trim() && !running) ask(draft);
          }}
        >
          <FluxMark size={32} spinning={running} />
          <input
            ref={input}
            value={draft}
            onChange={(e) => useAssistant.getState().setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "ArrowUp" && history.length) {
                e.preventDefault();
                const i = recall === null ? history.length - 1 : Math.max(0, recall - 1);
                setRecall(i);
                useAssistant.getState().setDraft(history[i] ?? "");
              }
            }}
            aria-label="Describe what you want"
            placeholder={`Try “${EXAMPLES[hint]?.text}”`}
            className="min-w-0 flex-1 bg-transparent text-lg outline-none placeholder:text-muted/70"
          />
          <Button type="submit" variant="primary" icon={ArrowUp} disabled={!draft.trim() || running}>
            Ask
          </Button>
        </form>
        <div ref={scroller} className="min-h-0 flex-1 overflow-y-auto border-t hairline">
          {turns.length === 0 ? <Welcome onPick={ask} /> : turns.map((t) => <TurnView key={t.id} turn={t} />)}
        </div>
        <footer className="flex items-center gap-3 border-t hairline px-4 py-2 text-xs text-muted">
          <EngineLine />
          <div className="flex-1" />
          {turns.length > 0 && !running && (
            <button type="button" onClick={() => useAssistant.getState().clear()} className="hover:text-fg">
              Clear
            </button>
          )}
          <span className="flex items-center gap-1">
            <Kbd>Esc</Kbd> close
          </span>
        </footer>
      </section>
    </div>
  );
}
