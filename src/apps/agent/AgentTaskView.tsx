import React, { useEffect, useRef, useState } from 'react';
import { motion } from 'framer-motion';
import { Check, X, Loader2, Ban, SkipForward, AlertTriangle, Download, Maximize2, ShieldCheck } from 'lucide-react';
import type { AgentTask, StepRun, StepStatus } from '../../core/agent/types';
import { TIER_LABEL } from '../../core/agent/types';
import { getTool } from '../../core/agent/tools';
import { approveAllLow, approveStep, continueAfterFailure, denyStep, stopTask } from '../../core/agent/runner';
import { usePrefersReducedMotion } from '../../core/usePrefersReducedMotion';
import { ImageLightbox } from './ImageLightbox';

interface Props {
  task: AgentTask;
  /** Snapshots of finished tasks have no controls. */
  readOnly?: boolean;
}

const STATUS_LABEL: Record<StepStatus, string> = {
  pending: 'Waiting',
  awaiting_approval: 'Needs your approval',
  running: 'Running',
  succeeded: 'Done',
  failed: 'Failed',
  denied: 'Denied',
  skipped: 'Skipped',
  blocked: 'Blocked',
  cancelled: 'Cancelled',
};

const mono = 'font-mono';
const focusRing = 'focus:outline-none focus-visible:ring-2 focus-visible:ring-geist-link/60';

function tierColor(tier: number): string {
  if (tier <= 0) return '#15803d';
  if (tier === 1) return 'var(--link, #0070f3)';
  if (tier === 2) return '#b45309';
  return '#ee0000';
}

function describe(run: StepRun): string {
  const tool = getTool(run.step.tool);
  try {
    return tool ? tool.describe(run.step.args) : run.step.tool;
  } catch {
    return run.step.tool;
  }
}

function StatusIcon({ status, reduced }: { status: StepStatus; reduced: boolean }) {
  const cls = 'w-3.5 h-3.5 flex-shrink-0';
  switch (status) {
    case 'succeeded':
      return <Check className={cls} style={{ color: '#15803d' }} aria-hidden />;
    case 'running':
      return <Loader2 className={`${cls} ${reduced ? '' : 'animate-spin'}`} style={{ color: 'var(--link, #0070f3)' }} aria-hidden />;
    case 'failed':
      return <AlertTriangle className={cls} style={{ color: '#ee0000' }} aria-hidden />;
    case 'blocked':
      return <Ban className={cls} style={{ color: '#ee0000' }} aria-hidden />;
    case 'denied':
    case 'cancelled':
      return <X className={cls} style={{ color: 'var(--text-mute, #8f8f8f)' }} aria-hidden />;
    case 'skipped':
      return <SkipForward className={cls} style={{ color: 'var(--text-mute, #8f8f8f)' }} aria-hidden />;
    case 'awaiting_approval':
      return <ShieldCheck className={cls} style={{ color: '#b45309' }} aria-hidden />;
    default:
      return <span className="w-2 h-2 rounded-full border flex-shrink-0 mx-[3px]" style={{ borderColor: 'var(--text-faint, #a1a1a1)' }} aria-hidden />;
  }
}

function TierBadge({ tier }: { tier: number }) {
  return (
    <span
      className={`px-1.5 py-0.5 rounded-[6px] border ${mono} text-[9px] uppercase font-bold tracking-wider`}
      style={{ borderColor: 'var(--hairline, #ebebeb)', color: tierColor(tier) }}
    >
      Tier {tier} {TIER_LABEL[tier as 0 | 1 | 2 | 3 | 4]}
    </span>
  );
}

const ArgsBlock: React.FC<{ args: Record<string, string> }> = ({ args }) => {
  const entries = Object.entries(args);
  if (entries.length === 0) return null;
  return (
    <pre
      className={`p-2 rounded-[6px] border whitespace-pre-wrap break-words text-[10px] leading-relaxed ${mono}`}
      style={{ background: 'var(--canvas, #fafafa)', borderColor: 'var(--hairline, #ebebeb)', color: 'var(--ink, #171717)' }}
    >
      {entries.map(([k, v]) => `${k}: ${v}`).join('\n')}
    </pre>
  );
};

const StepResult: React.FC<{ run: StepRun; index: number }> = ({ run, index }) => {
  const [enlarged, setEnlarged] = useState(false);
  const r = run.result;
  if (!r) return null;
  const text = r.ok ? r.text : r.error || r.text;
  const alt = `Screenshot from step ${index + 1}`;
  return (
    <div className="mt-1.5 space-y-1.5">
      {text && (
        <pre
          className={`p-2 rounded-[6px] border whitespace-pre-wrap break-words max-h-36 overflow-y-auto text-[10px] leading-relaxed ${mono}`}
          style={{
            background: 'var(--canvas, #fafafa)',
            borderColor: 'var(--hairline, #ebebeb)',
            color: r.ok ? 'var(--text-body, #4d4d4d)' : '#ee0000',
          }}
        >
          {text}
        </pre>
      )}
      {r.note && (
        <p className="text-[10px]" style={{ color: 'var(--text-mute, #8f8f8f)' }}>
          {r.note}
        </p>
      )}
      {r.imageDataUrl && (
        <div className="space-y-1.5">
          <img
            src={r.imageDataUrl}
            alt={alt}
            className="w-full max-h-64 object-contain rounded-[6px] border"
            style={{ borderColor: 'var(--hairline, #ebebeb)', background: 'var(--canvas, #fafafa)' }}
          />
          <div className="flex items-center space-x-2">
            <a
              href={r.imageDataUrl}
              download={`morphos-screenshot-${index + 1}.png`}
              aria-label={`Download screenshot from step ${index + 1}`}
              className={`inline-flex items-center space-x-1 px-2 py-1 rounded-[6px] border text-[11px] hover:bg-geist-hairlineSoft ${focusRing}`}
              style={{ borderColor: 'var(--hairline, #ebebeb)', color: 'var(--text-body, #4d4d4d)' }}
            >
              <Download className="w-3 h-3" aria-hidden />
              <span>Download</span>
            </a>
            <button
              type="button"
              onClick={() => setEnlarged(true)}
              aria-label={`Enlarge screenshot from step ${index + 1}`}
              className={`inline-flex items-center space-x-1 px-2 py-1 rounded-[6px] border text-[11px] hover:bg-geist-hairlineSoft ${focusRing}`}
              style={{ borderColor: 'var(--hairline, #ebebeb)', color: 'var(--text-body, #4d4d4d)' }}
            >
              <Maximize2 className="w-3 h-3" aria-hidden />
              <span>Enlarge</span>
            </button>
          </div>
          {enlarged && <ImageLightbox src={r.imageDataUrl} alt={alt} onClose={() => setEnlarged(false)} />}
        </div>
      )}
    </div>
  );
};

const ApprovalCard: React.FC<{ task: AgentTask; reduced: boolean }> = ({ task, reduced }) => {
  const run = task.steps[task.current];
  const approveRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    approveRef.current?.focus({ preventScroll: true });
  }, [task.id, task.current]);
  if (!run) return null;
  const offerAllLow = task.steps.some((s, i) => i >= task.current && s.step.tier <= 1 && (s.status === 'pending' || s.status === 'awaiting_approval'))
    && run.step.tier <= 1;
  const btn = `px-3 py-1.5 rounded-[6px] border text-[11px] font-semibold ${focusRing}`;
  return (
    <motion.div
      key={`${task.id}-${task.current}`}
      initial={reduced ? false : { opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: reduced ? 0 : 0.18 }}
      role="group"
      aria-label={`Approval needed for step ${task.current + 1} of ${task.steps.length}`}
      data-agent-approval-card
      className="rounded-[12px] border p-3 space-y-2"
      style={{ borderColor: tierColor(run.step.tier), background: 'var(--canvas-elevated, #ffffff)' }}
    >
      <div className="flex items-center justify-between">
        <span className={`${mono} text-[10px] uppercase tracking-wider font-bold`} style={{ color: 'var(--text-mute, #8f8f8f)' }}>
          Step {task.current + 1} of {task.steps.length}
        </span>
        <TierBadge tier={run.step.tier} />
      </div>
      <div className="text-xs font-semibold" style={{ color: 'var(--ink, #171717)' }}>
        <span className={mono} style={{ color: 'var(--link, #0070f3)' }}>{run.step.tool}</span>
        <span>: {describe(run)}</span>
      </div>
      <ArgsBlock args={run.step.args} />
      {run.step.reason && (
        <p className="text-[11px]" style={{ color: 'var(--text-mute, #8f8f8f)' }}>
          Why: {run.step.reason}
        </p>
      )}
      <div className="flex flex-wrap items-center gap-2 pt-1">
        <button
          ref={approveRef}
          type="button"
          onClick={approveStep}
          aria-label={`Approve step ${task.current + 1}: ${run.step.tool}`}
          className={btn}
          style={{ background: 'var(--accent, #171717)', color: '#ffffff', borderColor: 'var(--accent, #171717)' }}
        >
          Approve
        </button>
        <button
          type="button"
          onClick={() => denyStep('skip')}
          aria-label={`Deny and skip step ${task.current + 1}`}
          className={`${btn} hover:bg-geist-hairlineSoft`}
          style={{ borderColor: 'var(--hairline, #ebebeb)', color: 'var(--text-body, #4d4d4d)' }}
        >
          Deny (Skip step)
        </button>
        <button
          type="button"
          onClick={() => denyStep('stop')}
          aria-label="Stop task"
          className={`${btn} hover:bg-geist-hairlineSoft`}
          style={{ borderColor: 'var(--hairline, #ebebeb)', color: '#ee0000' }}
        >
          Stop task
        </button>
      </div>
      {offerAllLow && (
        <button
          type="button"
          onClick={approveAllLow}
          className={`text-[11px] underline ${focusRing}`}
          style={{ color: 'var(--link, #0070f3)' }}
        >
          Approve all remaining low-risk steps in this task
        </button>
      )}
    </motion.div>
  );
};

export const AgentTaskView: React.FC<Props> = ({ task, readOnly = false }) => {
  const reduced = usePrefersReducedMotion();
  const live = !readOnly;
  const awaiting = live && task.phase === 'awaiting_approval';
  const deciding = live && task.phase === 'awaiting_decision';
  const failedRun = deciding ? task.steps[task.current] : undefined;

  return (
    <div
      className="space-y-2.5"
      data-agent-task={task.id}
      data-agent-phase={task.phase}
      role="group"
      aria-label="Agent task"
      aria-live={live ? 'polite' : undefined}
    >
      {task.phase === 'planning' && live ? (
        <div className={`flex items-center justify-between ${mono} text-[11px]`} style={{ color: 'var(--text-mute, #8f8f8f)' }}>
          <span className="flex items-center space-x-2">
            <Loader2 className={`w-3.5 h-3.5 ${reduced ? '' : 'animate-spin'}`} aria-hidden />
            <span>Planning...</span>
          </span>
          <button
            type="button"
            onClick={stopTask}
            aria-label="Stop task"
            className={`px-2 py-1 rounded-[6px] border text-[11px] hover:bg-geist-hairlineSoft ${focusRing}`}
            style={{ borderColor: 'var(--hairline, #ebebeb)', color: '#ee0000' }}
          >
            Stop
          </button>
        </div>
      ) : (
        <>
          {task.summary && (
            <div className="text-xs font-semibold" style={{ color: 'var(--ink, #171717)' }}>
              {task.summary}
            </div>
          )}
          {task.notes.length > 0 && (
            <ul className="text-[10px] space-y-0.5 list-disc pl-4" style={{ color: 'var(--text-mute, #8f8f8f)' }}>
              {task.notes.map((n, i) => (
                <li key={i}>{n}</li>
              ))}
            </ul>
          )}
          {task.error && (
            <p className="text-[11px]" style={{ color: '#ee0000' }}>
              {task.error}
            </p>
          )}
          <ol className="space-y-2">
            {task.steps.map((run, i) => {
              const isCurrentApproval = awaiting && i === task.current && run.status === 'awaiting_approval';
              return (
                <li key={run.step.id} data-agent-step-status={run.status}>
                  <div className="flex items-start space-x-2">
                    <span className="mt-0.5">
                      <StatusIcon status={run.status} reduced={reduced} />
                    </span>
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center justify-between gap-2">
                        <span className="text-[11px]" style={{ color: 'var(--ink, #171717)' }}>
                          {i + 1}. <span className={mono}>{run.step.tool}</span>: {describe(run)}
                        </span>
                        <span className="flex items-center gap-1.5 flex-shrink-0">
                          <TierBadge tier={run.step.tier} />
                          <span className={`${mono} text-[9px] uppercase`} style={{ color: 'var(--text-mute, #8f8f8f)' }}>
                            {STATUS_LABEL[run.status]}
                            {run.approvedBy === 'auto-low' ? ' (auto)' : ''}
                          </span>
                        </span>
                      </div>
                      {run.status === 'blocked' && run.step.blockReason && (
                        <p className="text-[10px] mt-0.5" style={{ color: '#ee0000' }}>
                          {run.step.blockReason}
                        </p>
                      )}
                      {!isCurrentApproval && <StepResult run={run} index={i} />}
                    </div>
                  </div>
                  {isCurrentApproval && (
                    <div className="mt-2">
                      <ApprovalCard task={task} reduced={reduced} />
                    </div>
                  )}
                </li>
              );
            })}
          </ol>
          {deciding && failedRun && (
            <div
              role="group"
              aria-label="Step failed, choose what to do"
              className="rounded-[12px] border p-3 space-y-2"
              style={{ borderColor: '#ee0000', background: 'var(--canvas-elevated, #ffffff)' }}
            >
              <p className="text-[11px]" style={{ color: '#ee0000' }}>
                Step {task.current + 1} failed: {failedRun.result?.error || 'Unknown error'}
              </p>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={continueAfterFailure}
                  className={`px-3 py-1.5 rounded-[6px] border text-[11px] font-semibold ${focusRing}`}
                  style={{ background: 'var(--accent, #171717)', color: '#ffffff', borderColor: 'var(--accent, #171717)' }}
                >
                  Continue with next step
                </button>
                <button
                  type="button"
                  onClick={stopTask}
                  aria-label="Stop task"
                  className={`px-3 py-1.5 rounded-[6px] border text-[11px] font-semibold hover:bg-geist-hairlineSoft ${focusRing}`}
                  style={{ borderColor: 'var(--hairline, #ebebeb)', color: '#ee0000' }}
                >
                  Stop task
                </button>
              </div>
            </div>
          )}
          {task.finalMessage && (
            <div className="text-xs whitespace-pre-wrap" style={{ color: 'var(--text-body, #4d4d4d)' }}>
              {task.finalMessage}
            </div>
          )}
        </>
      )}
    </div>
  );
};
