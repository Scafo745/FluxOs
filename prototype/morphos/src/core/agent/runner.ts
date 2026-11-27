import { useOSStore } from '../store';
import type { CloudConfig } from '../types';
import { planTask } from './planner';
import { isTerminalPhase } from './machine';
import { getTool, tierFor } from './tools';
import { validateArgs } from './validate';
import type { AgentTask, PlanStep, StepRun, ToolResult } from './types';

/**
 * Module-level orchestrator. All task state lives in the Zustand store
 * (agentTask); this file only plans once, executes the approved current step,
 * and reports results back as reducer actions.
 */

const MAX_TEXT = 4000;
const PNG_PREFIX = 'data:image/png;base64,';
const MAX_IMAGE_CHARS = 12_000_000;

const inFlight = new Set<string>();
const controllers = new Map<string, AbortController>();
const scratchByTask = new Map<string, Record<string, unknown>>();

const store = () => useOSStore.getState();
const dispatch = (a: Parameters<ReturnType<typeof store>['agentDispatch']>[0]) => store().agentDispatch(a);

function clip(s: string, n: number): string {
  return s.length > n ? `${s.slice(0, n)}...` : s;
}

function auditStep(step: PlanStep, status: 'ALLOWED' | 'BLOCKED', reason: string, severity: 'INFO' | 'WARNING' | 'CRITICAL' = 'INFO') {
  store().addAuditLog(`agent.tool ${step.tool} ${clip(JSON.stringify(step.args), 160)}`, status, clip(reason, 160), severity);
}

function sanitizeResult(res: ToolResult): ToolResult {
  const out: ToolResult = { ok: !!res.ok };
  if (typeof res.text === 'string') out.text = clip(res.text, MAX_TEXT);
  if (typeof res.error === 'string') out.error = clip(res.error, MAX_TEXT);
  if (typeof res.note === 'string') out.note = clip(res.note, 500);
  if (typeof res.imageDataUrl === 'string' && res.imageDataUrl.startsWith(PNG_PREFIX) && res.imageDataUrl.length < MAX_IMAGE_CHARS) {
    out.imageDataUrl = res.imageDataUrl;
  }
  return out;
}

function currentRun(task: AgentTask | null): StepRun | null {
  if (!task || task.phase !== 'running') return null;
  const run = task.steps[task.current];
  return run && run.status === 'running' ? run : null;
}

async function execute(task: AgentTask, index: number, run: StepRun, key: string): Promise<void> {
  const step = run.step;
  const controller = new AbortController();
  controllers.set(task.id, controller);
  const fail = (error: string) => {
    auditStep(step, 'BLOCKED', error, 'WARNING');
    dispatch({ type: 'STEP_FAILED', index, result: { ok: false, error } });
  };

  try {
    const tool = getTool(step.tool);
    if (!tool) return fail('Tool is not registered.');
    // Re-validate and recompute the tier right before running.
    const v = validateArgs(tool.argsSchema, step.args);
    if (!v.ok) return fail(`Invalid arguments: ${v.errors.join('; ')}`);
    const tier = tierFor(tool, v.value);
    if (tier >= 4) return fail('Blocked: this step is never executed.');
    if (tier > step.tier) return fail('Risk level changed since approval. Please re-plan.');

    auditStep(step, 'ALLOWED', `Approved by ${run.approvedBy === 'auto-low' ? `auto-low (${store().profile.userRole})` : 'user'}`);

    let scratch = scratchByTask.get(task.id);
    if (!scratch) {
      scratch = {};
      scratchByTask.set(task.id, scratch);
    }

    let timer: ReturnType<typeof setTimeout> | undefined;
    const timeout = new Promise<ToolResult>((resolve) => {
      timer = setTimeout(() => {
        controller.abort();
        resolve({ ok: false, error: `Timed out after ${Math.round(tool.timeoutMs / 1000)}s.` });
      }, tool.timeoutMs);
    });
    let res: ToolResult;
    try {
      res = await Promise.race([tool.execute(v.value, { signal: controller.signal, taskId: task.id, scratch }), timeout]);
    } finally {
      clearTimeout(timer);
    }

    const result = sanitizeResult(res);
    if (result.imageDataUrl) scratch.lastImage = result.imageDataUrl;
    const live = store().agentTask;
    if (!live || live.id !== task.id || live.phase !== 'running') return; // stopped meanwhile

    if (result.ok) {
      auditStep(step, 'ALLOWED', `Result: ${result.text || 'ok'}`.replace(/data:image\S+/g, '[image]'));
      dispatch({ type: 'STEP_SUCCEEDED', index, result });
    } else {
      auditStep(step, 'BLOCKED', `Failed: ${result.error || 'unknown error'}`, 'WARNING');
      dispatch({ type: 'STEP_FAILED', index, result });
    }
  } catch (err) {
    const live = store().agentTask;
    if (live && live.id === task.id && live.phase === 'running') {
      fail(err instanceof Error ? err.message : 'Unexpected error.');
    }
  } finally {
    controllers.delete(task.id);
    inFlight.delete(key);
  }
}

/** Executes the current step when the task is in the running phase. Guarded to one run per step. */
function pump(): void {
  const task = store().agentTask;
  const run = currentRun(task);
  if (!task || !run) return;
  const key = `${task.id}:${run.step.id}`;
  if (inFlight.has(key)) return;
  inFlight.add(key);
  void execute(task, task.current, run, key);
}

// Any task change may have advanced to a running step (approval, auto-low, mode switch).
let lastTask: AgentTask | null = null;

/** Keeps the Copilot window above windows the agent opened, so prompts are never hidden. */
function surfaceCopilot(prev: AgentTask | null, next: AgentTask | null): void {
  if (!next || !['awaiting_approval', 'awaiting_decision', 'done'].includes(next.phase)) return;
  if (prev && prev.id === next.id && prev.phase === next.phase && prev.current === next.current) return;
  const copilot = store().windows.find((w) => w.appId === 'copilot');
  if (copilot) store().focusWindow(copilot.id);
}

useOSStore.subscribe((s) => {
  if (s.agentTask === lastTask) return;
  const prev = lastTask;
  lastTask = s.agentTask;
  pump();
  surfaceCopilot(prev, s.agentTask);
});

function activeTask(): AgentTask | null {
  const t = store().agentTask;
  return t && !isTerminalPhase(t.phase) ? t : null;
}

export function hasActiveTask(): boolean {
  return activeTask() !== null;
}

export async function startAgentTask(prompt: string, cloudConfig: CloudConfig): Promise<void> {
  if (activeTask()) stopTask();
  const id = `task-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
  dispatch({ type: 'START', id, prompt, autoLow: store().agentMode === 'auto-low' });
  try {
    const plan = await planTask(prompt, cloudConfig);
    const live = store().agentTask;
    if (!live || live.id !== id || live.phase !== 'planning') return;
    dispatch({ type: 'PLAN_READY', taskId: id, summary: plan.summary, steps: plan.steps, notes: plan.notes, planner: plan.planner });
    store().addAuditLog(`agent.plan ${clip(prompt, 100)}`, 'ALLOWED', `${plan.steps.length} step(s) planned by ${plan.planner} planner`);
    for (const step of plan.steps) {
      if (step.tier >= 4) auditStep(step, 'BLOCKED', step.blockReason || 'Blocked by policy.', 'CRITICAL');
    }
  } catch (err) {
    const live = store().agentTask;
    if (live && live.id === id && live.phase === 'planning') {
      dispatch({ type: 'PLAN_FAILED', taskId: id, error: err instanceof Error ? err.message : 'Planning failed.' });
    }
  }
}

export function approveStep(index?: number): void {
  const t = store().agentTask;
  if (!t || t.phase !== 'awaiting_approval') return;
  dispatch({ type: 'APPROVE', by: 'user', index: index ?? t.current });
}

export function approveAllLow(): void {
  dispatch({ type: 'APPROVE_ALL_LOW' });
}

export function denyStep(mode: 'skip' | 'stop', index?: number): void {
  const t = store().agentTask;
  if (!t || t.phase !== 'awaiting_approval') return;
  if (index !== undefined && index !== t.current) return;
  const run = t.steps[t.current];
  if (run) auditStep(run.step, 'BLOCKED', mode === 'stop' ? 'Denied by user, task stopped' : 'Denied by user, step skipped');
  dispatch({ type: 'DENY', mode, index: index ?? t.current });
}

export function continueAfterFailure(): void {
  dispatch({ type: 'CONTINUE' });
}

export function stopTask(): void {
  const t = activeTask();
  if (!t) return;
  controllers.get(t.id)?.abort();
  store().addAuditLog('agent.stop', 'BLOCKED', 'Task stopped by user');
  dispatch({ type: 'STOP' });
}

export function dismissTask(): void {
  const t = store().agentTask;
  if (t) {
    scratchByTask.delete(t.id);
    if (!isTerminalPhase(t.phase)) stopTask();
  }
  dispatch({ type: 'RESET' });
}
