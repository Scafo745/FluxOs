import type { AgentAction, AgentTask, StepRun } from './types';

/**
 * Pure task state machine. Rules enforced here:
 * - tier 4 steps become `blocked` and are never run
 * - tier >= 2 steps always wait for an explicit user approval
 * - auto approval only happens when task.autoLow && tier <= 1
 * - actions carrying a stale index or arriving in the wrong phase are ignored
 */

const TERMINAL = ['done', 'failed', 'cancelled'] as const;

export function isTerminalPhase(phase: AgentTask['phase']): boolean {
  return (TERMINAL as readonly string[]).includes(phase);
}

function setStep(task: AgentTask, index: number, patch: Partial<StepRun>): AgentTask {
  return { ...task, steps: task.steps.map((s, i) => (i === index ? { ...s, ...patch } : s)) };
}

function buildFinalMessage(task: AgentTask, lead: string): string {
  const count = (st: StepRun['status']) => task.steps.filter((s) => s.status === st).length;
  const parts: string[] = [];
  const pairs: [string, number][] = [
    ['completed', count('succeeded')],
    ['skipped', count('denied') + count('skipped')],
    ['blocked', count('blocked')],
    ['failed', count('failed')],
    ['cancelled', count('cancelled')],
  ];
  for (const [label, n] of pairs) if (n > 0) parts.push(`${n} ${label}`);
  const tally = parts.length > 0 ? parts.join(', ') : 'no steps ran';
  return `${lead} ${tally}.`;
}

/** Walks forward from task.current and settles on the next step that needs attention. */
function advance(input: AgentTask): AgentTask {
  let task = input;
  for (let i = task.current; i < task.steps.length; i++) {
    const run = task.steps[i];
    if (run.status !== 'pending') continue;
    const tier = run.step.tier;

    if (tier >= 4) {
      task = setStep(task, i, { status: 'blocked' });
      continue;
    }
    if (task.autoLow && tier <= 1) {
      task = setStep(task, i, { status: 'running', approvedBy: 'auto-low' });
      return { ...task, current: i, phase: 'running' };
    }
    task = setStep(task, i, { status: 'awaiting_approval' });
    return { ...task, current: i, phase: 'awaiting_approval' };
  }
  const finished: AgentTask = { ...task, current: task.steps.length, phase: 'done' };
  return { ...finished, finalMessage: buildFinalMessage(finished, 'Finished:') };
}

function cancelRest(task: AgentTask, fromIndex: number): AgentTask {
  const steps = task.steps.map((s, i) =>
    i >= fromIndex && (s.status === 'pending' || s.status === 'awaiting_approval' || s.status === 'running')
      ? { ...s, status: (s.step.tier >= 4 ? 'blocked' : 'cancelled') as StepRun['status'] }
      : s
  );
  return { ...task, steps };
}

export function reduceTask(task: AgentTask | null, action: AgentAction): AgentTask | null {
  if (action.type === 'RESET') return null;

  if (action.type === 'START') {
    return {
      id: action.id,
      prompt: action.prompt,
      phase: 'planning',
      summary: '',
      planner: 'local',
      notes: [],
      steps: [],
      current: 0,
      autoLow: action.autoLow,
    };
  }

  if (!task || isTerminalPhase(task.phase)) return task;

  switch (action.type) {
    case 'PLAN_READY': {
      if (task.phase !== 'planning' || action.taskId !== task.id) return task;
      const steps: StepRun[] = action.steps.map((step) => ({ step, status: 'pending' }));
      return advance({
        ...task,
        summary: action.summary,
        planner: action.planner,
        notes: action.notes,
        steps,
        current: 0,
      });
    }

    case 'PLAN_FAILED': {
      if (task.phase !== 'planning' || action.taskId !== task.id) return task;
      return { ...task, phase: 'failed', error: action.error, finalMessage: `Could not plan this request. ${action.error}` };
    }

    case 'APPROVE': {
      if (task.phase !== 'awaiting_approval') return task;
      const idx = action.index ?? task.current;
      const run = task.steps[idx];
      if (idx !== task.current || !run || run.status !== 'awaiting_approval') return task;
      if (run.step.tier >= 4) return task;
      // Auto approval is only legal for low tiers on an auto-low task.
      if (action.by === 'auto-low' && !(task.autoLow && run.step.tier <= 1)) return task;
      return { ...setStep(task, idx, { status: 'running', approvedBy: action.by }), phase: 'running' };
    }

    case 'APPROVE_ALL_LOW': {
      if (task.phase === 'planning') return task;
      let next: AgentTask = { ...task, autoLow: true };
      if (next.phase === 'awaiting_approval') {
        const run = next.steps[next.current];
        if (run && run.status === 'awaiting_approval' && run.step.tier <= 1) {
          next = { ...setStep(next, next.current, { status: 'running', approvedBy: 'user' }), phase: 'running' };
        }
      }
      return next;
    }

    case 'SET_AUTO_LOW': {
      let next: AgentTask = { ...task, autoLow: action.value };
      if (action.value && next.phase === 'awaiting_approval') {
        const run = next.steps[next.current];
        if (run && run.status === 'awaiting_approval' && run.step.tier <= 1) {
          next = { ...setStep(next, next.current, { status: 'running', approvedBy: 'auto-low' }), phase: 'running' };
        }
      }
      return next;
    }

    case 'DENY': {
      if (task.phase !== 'awaiting_approval') return task;
      const idx = action.index ?? task.current;
      const run = task.steps[idx];
      if (idx !== task.current || !run || run.status !== 'awaiting_approval') return task;
      const denied = setStep(task, idx, { status: 'denied' });
      if (action.mode === 'stop') {
        const stopped = { ...cancelRest(denied, idx + 1), phase: 'cancelled' as const };
        return { ...stopped, finalMessage: buildFinalMessage(stopped, 'Stopped by you:') };
      }
      return advance({ ...denied, current: idx + 1 });
    }

    case 'STEP_SUCCEEDED': {
      if (task.phase !== 'running' || action.index !== task.current) return task;
      if (task.steps[action.index]?.status !== 'running') return task;
      const done = setStep(task, action.index, { status: 'succeeded', result: action.result });
      return advance({ ...done, current: action.index + 1 });
    }

    case 'STEP_FAILED': {
      if (task.phase !== 'running' || action.index !== task.current) return task;
      if (task.steps[action.index]?.status !== 'running') return task;
      const failed = setStep(task, action.index, { status: 'failed', result: action.result });
      return { ...failed, phase: 'awaiting_decision' };
    }

    case 'CONTINUE': {
      if (task.phase !== 'awaiting_decision') return task;
      return advance({ ...task, current: task.current + 1 });
    }

    case 'STOP': {
      const stopped = { ...cancelRest(task, 0), phase: 'cancelled' as const };
      // cancelRest only touches unfinished steps, so completed work stays recorded.
      return { ...stopped, finalMessage: buildFinalMessage(stopped, 'Stopped by you:') };
    }

    default:
      return task;
  }
}
