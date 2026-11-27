/**
 * Agent Mode shared types. Tiers: 0 Safe, 1 Low, 2 Medium, 3 High, 4 Blocked.
 * Tier 4 steps are never executed.
 */
export type RiskTier = 0 | 1 | 2 | 3 | 4;

export const TIER_LABEL: Record<RiskTier, string> = {
  0: 'Safe',
  1: 'Low',
  2: 'Medium',
  3: 'High',
  4: 'Blocked',
};

export interface ToolResult {
  ok: boolean;
  text?: string;
  imageDataUrl?: string;
  error?: string;
  note?: string;
}

export type FieldSpec = {
  type: 'string' | 'enum';
  required?: boolean;
  maxLen?: number;
  pattern?: RegExp;
  values?: readonly string[];
};

export type ArgsSchema = Record<string, FieldSpec>;
export type ToolArgs = Record<string, string>;

export interface ToolContext {
  signal: AbortSignal;
  taskId: string;
  scratch: Record<string, unknown>;
}

export interface ToolDef {
  name: string;
  description: string;
  tier: RiskTier | ((args: ToolArgs) => RiskTier);
  /** Plain reason shown when the tier is 4. */
  blockReason?: (args: ToolArgs) => string | undefined;
  argsSchema: ArgsSchema;
  timeoutMs: number;
  describe: (args: ToolArgs) => string;
  execute: (args: ToolArgs, ctx: ToolContext) => Promise<ToolResult>;
}

export interface PlanStep {
  id: string;
  tool: string;
  args: ToolArgs;
  reason: string;
  tier: RiskTier;
  source: 'local' | 'model';
  blockReason?: string;
}

export type AgentMode = 'ask-every-time' | 'auto-low';

export type StepStatus =
  | 'pending'
  | 'awaiting_approval'
  | 'running'
  | 'succeeded'
  | 'failed'
  | 'denied'
  | 'skipped'
  | 'blocked'
  | 'cancelled';

export type TaskPhase =
  | 'planning'
  | 'awaiting_approval'
  | 'running'
  | 'awaiting_decision'
  | 'done'
  | 'failed'
  | 'cancelled';

export interface StepRun {
  step: PlanStep;
  status: StepStatus;
  approvedBy?: 'user' | 'auto-low';
  result?: ToolResult;
}

export interface AgentTask {
  id: string;
  prompt: string;
  phase: TaskPhase;
  summary: string;
  planner: 'local' | 'llm';
  notes: string[];
  steps: StepRun[];
  /** Index of the step currently being handled. */
  current: number;
  autoLow: boolean;
  finalMessage?: string;
  error?: string;
}

export type AgentAction =
  | { type: 'START'; id: string; prompt: string; autoLow: boolean }
  | { type: 'PLAN_READY'; taskId: string; summary: string; steps: PlanStep[]; notes: string[]; planner: 'local' | 'llm' }
  | { type: 'PLAN_FAILED'; taskId: string; error: string }
  | { type: 'APPROVE'; by: 'user' | 'auto-low'; index?: number }
  | { type: 'APPROVE_ALL_LOW' }
  | { type: 'DENY'; mode: 'skip' | 'stop'; index?: number }
  | { type: 'STEP_SUCCEEDED'; index: number; result: ToolResult }
  | { type: 'STEP_FAILED'; index: number; result: ToolResult }
  | { type: 'CONTINUE' }
  | { type: 'STOP' }
  | { type: 'SET_AUTO_LOW'; value: boolean }
  | { type: 'RESET' };
