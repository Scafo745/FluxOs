import { useOSStore } from '../store';
import { hasUsableProviderForMode, runProvidersForText } from '../ai-chat';
import type { CloudConfig } from '../types';
import { getTool, listTools, resolveAppName, schemaSummary, tierFor } from './tools';
import { validateArgs } from './validate';
import type { PlanStep, ToolArgs } from './types';

export const MAX_PLAN_STEPS = 8;

export interface PlanResult {
  summary: string;
  steps: PlanStep[];
  notes: string[];
  planner: 'local' | 'llm';
}

interface RawStep {
  tool: unknown;
  args: unknown;
  reason?: unknown;
}

interface PlannerCtx {
  files: { name: string; path: string; type: string }[];
  cwd: string;
}

function defaultCtx(): PlannerCtx {
  const s = useOSStore.getState();
  return { files: s.filesystem, cwd: s.currentPath };
}

/** Replaces control characters (code < 32) with spaces and trims. */
function oneLine(s: string): string {
  return Array.from(s, (ch) => (ch.charCodeAt(0) < 32 ? ' ' : ch)).join('').trim();
}

// ───────────────────────── validation (untrusted input) ─────────────────────────

/**
 * Turns raw (untrusted) steps into PlanSteps. Unknown tools and invalid args are
 * dropped with a visible note. Tiers are always recomputed here, never trusted.
 */
export function validatePlan(
  raw: unknown,
  source: 'local' | 'model'
): { steps: PlanStep[]; notes: string[]; summary: string } {
  const notes: string[] = [];
  const steps: PlanStep[] = [];
  const obj = raw && typeof raw === 'object' ? (raw as { summary?: unknown; steps?: unknown }) : {};
  const list: unknown[] = Array.isArray(obj.steps) ? obj.steps : [];

  for (const item of list) {
    const rs = (item && typeof item === 'object' ? item : {}) as RawStep;
    const toolName = typeof rs.tool === 'string' ? rs.tool : '';
    const tool = getTool(toolName);
    if (!tool) {
      notes.push(`Dropped a step: "${toolName.slice(0, 40) || 'unnamed'}" is not an available tool.`);
      continue;
    }
    if (steps.length >= MAX_PLAN_STEPS) {
      notes.push(`Dropped "${tool.name}": plans are limited to ${MAX_PLAN_STEPS} steps.`);
      continue;
    }
    const v = validateArgs(tool.argsSchema, rs.args);
    if (!v.ok) {
      notes.push(`Dropped ${tool.name}: ${v.errors.join('; ')}.`);
      continue;
    }
    const tier = tierFor(tool, v.value);
    const reason = typeof rs.reason === 'string' && rs.reason.trim() ? oneLine(rs.reason).slice(0, 200) : 'Part of your request';
    steps.push({
      id: `s${steps.length + 1}`,
      tool: tool.name,
      args: v.value,
      reason,
      tier,
      source,
      blockReason: tier >= 4 ? tool.blockReason?.(v.value) || 'Blocked by policy.' : undefined,
    });
  }

  const summary =
    typeof obj.summary === 'string' && obj.summary.trim()
      ? oneLine(obj.summary).slice(0, 200)
      : `${steps.length} step${steps.length === 1 ? '' : 's'}`;
  return { steps, notes, summary };
}

// ───────────────────────── local deterministic planner ─────────────────────────

const ACTION_VERB =
  /^(?:open|launch|start|run|execute|exec|search|google|look\s*up|go\s+to|visit|navigate|browse|head\s+to|take|capture|grab|snap|screenshot|screen\s*shot|show|display|let\s+me\s+see|read|cat|install|switch|change|set|close|quit|exit|focus|bring)\b/i;

const PREFIX =
  /^(?:(?:please|kindly|hey|ok|okay)[\s,]+)*(?:(?:can|could|would|will)\s+you\s+(?:please\s+)?|i\s+want\s+you\s+to\s+|i(?:'d|\s+would)\s+like\s+you\s+to\s+|i\s+need\s+you\s+to\s+|go\s+ahead\s+and\s+|you\s+should\s+)?(?:please\s+)?/i;

const SEP = /(\s*[,;]\s*|\s*\.\s+|\s+(?:and\s+then|then|after\s+that|afterwards|and)\s+)/i;
const LEADING_CONNECTOR = /^(?:(?:and\s+)?then|after\s+that|afterwards|and)\s+/i;

function cleanClause(c: string): string {
  let s = c.trim();
  for (let i = 0; i < 3; i++) {
    const before = s;
    s = s.replace(LEADING_CONNECTOR, '').replace(PREFIX, '').trim();
    if (s === before) break;
  }
  return s.replace(/\s+(?:for me|please|thanks|thank you)$/i, '').replace(/[?!.\s]+$/, '').trim();
}

/** Splits a message into clauses. A split only happens when the next part starts with an action verb. */
export function splitClauses(prompt: string): string[] {
  const parts = prompt.split(SEP);
  const clauses: string[] = [];
  let current = parts[0] ?? '';
  for (let i = 1; i < parts.length; i += 2) {
    const sep = parts[i];
    const next = parts[i + 1] ?? '';
    if (ACTION_VERB.test(cleanClause(next))) {
      clauses.push(current);
      current = next;
    } else {
      current += sep + next;
    }
  }
  clauses.push(current);
  return clauses.map(cleanClause).filter(Boolean);
}

const IMAGE_REF =
  /^(?:me\s+)?(?:it|that|this|these|the\s+(?:screenshot|screen\s*shot|image|picture|result|capture|results))(?:\s+(?:to\s+me|here|too|also))?$|^me$|^$|^to\s+me$/i;

function stripQuotes(s: string): string {
  const m = s.trim().match(/^(?:"([^"]*)"|'([^']*)'|`([^`]*)`)$/);
  return m ? (m[1] ?? m[2] ?? m[3] ?? '').trim() : s.trim();
}

function looksLikeUrl(s: string): boolean {
  return /^(?:https?:\/\/\S+|morphos:\/\/\S*|[a-z0-9-]+(?:\.[a-z0-9-]+)*\.(?:com|org|net|io|dev|ai|app|co|in|edu|gov|info|me|xyz)(?:[/?#]\S*)?)$/i.test(s);
}

function resolveFile(token: string, ctx: PlannerCtx): string | null {
  const t = stripQuotes(token);
  if (!/^[A-Za-z0-9_./-]+$/.test(t)) return null;
  const files = ctx.files.filter((f) => f.type === 'file');
  if (t.includes('/')) return t;
  const matches = files.filter((f) => f.name.toLowerCase() === t.toLowerCase());
  if (matches.length > 0) {
    const inCwd = matches.find((f) => f.path === `${ctx.cwd === '/' ? '' : ctx.cwd}/${f.name}`);
    return (inCwd || matches[0]).path;
  }
  return /\.[A-Za-z0-9]{1,6}$/.test(t) ? t : null;
}

interface RawLocalStep {
  tool: string;
  args: ToolArgs;
  reason: string;
}

/**
 * Deterministic, offline planner. A clause only counts when it STARTS with an
 * action verb, so questions like "what is a screenshot" produce no steps.
 */
export function localPlan(prompt: string, ctx: PlannerCtx = defaultCtx()): PlanResult {
  // Protect quoted / backticked text so it is never split or mis-parsed.
  const quoted: string[] = [];
  const protectedText = prompt.replace(/`[^`]*`|"[^"]*"|“[^”]*”/g, (m) => {
    quoted.push(m);
    return `QQ${quoted.length - 1}QQ`;
  });
  const restore = (s: string) => s.replace(/QQ(\d+)QQ/g, (_, n: string) => quoted[Number(n)] ?? '');
  const unquote = (s: string) => stripQuotes(restore(s).replace(/^“|”$/g, '"'));

  const raw: RawLocalStep[] = [];
  let lastApp: string | null = null;
  let hasShot = false;
  const push = (tool: string, args: ToolArgs, reason: string) => raw.push({ tool, args, reason });

  for (const clause of splitClauses(protectedText)) {
    let m: RegExpMatchArray | null;

    // theme changes (kept ahead of "set"/"change" app handling)
    if (/^(?:switch|change|set)\b/i.test(clause) && /\b(?:theme|look|mode|appearance)\b/i.test(clause)) {
      push('set_theme', { prompt: unquote(clause) }, 'You asked to change the theme');
      continue;
    }

    // screenshot
    m =
      clause.match(/^(?:take|capture|grab|snap|get)\s+(?:a|an|the|another|one)?\s*(?:screenshot|screen\s*shot|screen\s*capture|snapshot)\b(.*)$/i) ||
      clause.match(/^(?:screenshot|screen\s*shot)\b(.*)$/i);
    const m2 = m ? null : clause.match(/^(?:capture|grab|snap)\s+(?:the\s+)?((?:screen|desktop|window|page|browser)\b.*)$/i);
    if (m || m2) {
      const rest = (m ? m[1] : m2![1]).replace(/^\s*(?:of|for|from)?\s*/i, '').replace(/^(?:the|a|my|this|that)\s+/i, '').trim();
      const restClean = unquote(rest);
      let app: string | null = null;
      let target: 'window' | 'desktop';
      if (/\b(?:desktop|screen|whole|entire|full)\b/i.test(restClean) && !resolveAppName(restClean)) {
        target = 'desktop';
      } else if ((app = resolveAppName(restClean))) {
        target = 'window';
      } else if (/\b(?:browser)\b/i.test(restClean)) {
        target = 'window';
        app = 'browser';
      } else {
        // pronouns, "page", "results", "window", empty
        app = lastApp;
        target = app ? 'window' : 'desktop';
      }
      push('screenshot', target === 'desktop' ? { target } : { target, app: app as string }, 'You asked for a screenshot');
      hasShot = true;
      continue;
    }

    // show / display
    m = clause.match(/^(?:show|display|let\s+me\s+see|present|send)\s*(.*)$/i);
    if (m) {
      const rest = restore(m[1]).trim();
      if (IMAGE_REF.test(rest)) {
        if (hasShot) push('show_image', { source: 'last' }, 'You asked to see the screenshot');
        continue;
      }
      const file = resolveFile(rest.replace(/^me\s+/i, '').replace(/^(?:the\s+)?(?:contents?\s+of\s+)?(?:the\s+)?(?:file\s+)?/i, ''), ctx);
      if (file) push('read_file', { path: file }, 'You asked to see this file');
      continue;
    }

    // read / cat
    m = clause.match(/^(?:read|cat)\s+(.+)$/i);
    if (m) {
      const file = resolveFile(restore(m[1]).replace(/^(?:the\s+)?(?:contents?\s+of\s+)?(?:the\s+)?(?:file\s+)?/i, ''), ctx);
      if (file) push('read_file', { path: file }, 'You asked to read this file');
      continue;
    }

    // search
    m = clause.match(/^(?:search(?:\s+(?:for|up))?|google|look\s*up|browse\s+for)\s+(.+)$/i);
    if (m) {
      let q = unquote(m[1]);
      q = q.replace(/^(?:the\s+)?(?:web|internet|google|browser)\s+for\s+/i, '').replace(/^for\s+/i, '');
      q = q.replace(/\s+(?:in|on|using|with|via)\s+(?:the\s+)?(?:browser|google|web|internet)$/i, '').trim();
      if (q) {
        push('browser_search', { query: q }, 'You asked to search');
        lastApp = 'browser';
      }
      continue;
    }

    // go to / visit / navigate
    m = clause.match(/^(?:go\s+to|visit|navigate\s+to|head\s+to|browse\s+to|browse)\s+(.+)$/i);
    if (m) {
      const target = unquote(m[1]).replace(/^the\s+/i, '');
      if (looksLikeUrl(target)) {
        push('browser_navigate', { url: target }, 'You asked to visit a page');
        lastApp = 'browser';
      } else {
        const app = resolveAppName(target);
        if (app) {
          push('open_app', { app }, `You asked to go to ${app}`);
          lastApp = app;
        }
      }
      continue;
    }

    // install
    m = clause.match(/^install\s+(?:the\s+)?(.+)$/i);
    if (m) {
      let rest = unquote(m[1]);
      let manager: string | undefined;
      const mm = rest.match(/\s+(?:with|using|via|through|by)\s+(npm|apt|pip)$/i);
      if (mm) {
        manager = mm[1].toLowerCase();
        rest = rest.slice(0, mm.index).trim();
      }
      rest = rest.replace(/\s+(?:packages?|library|module|lib)$/i, '').trim();
      if (/^[A-Za-z0-9@][A-Za-z0-9@._/-]*$/.test(rest)) {
        push('install_package', manager ? { name: rest, manager } : { name: rest }, 'You asked to install a package');
      }
      continue;
    }

    // run / execute
    m = clause.match(/^(?:run|execute|exec)\s+(?:the\s+)?(?:(?:command|cmd|script)\s+)?(.+)$/i);
    if (m) {
      let cmd = unquote(m[1]);
      cmd = cmd.replace(/\s+(?:in|on|using)\s+(?:the\s+)?(?:terminal|shell|console)$/i, '').trim();
      const app = resolveAppName(cmd);
      if (app) {
        push('open_app', { app }, `You asked to run ${app}`);
        lastApp = app;
      } else if (cmd) {
        push('run_shell', { command: cmd }, 'You asked to run a command');
      }
      continue;
    }

    // open / launch / start
    m = clause.match(/^(?:open|launch|start|fire\s+up|boot\s+up)\s+(?:up\s+)?(.+)$/i);
    if (m) {
      const target = unquote(m[1]).replace(/^(?:the|a|an|my)\s+/i, '');
      if (looksLikeUrl(target)) {
        push('browser_navigate', { url: target }, 'You asked to open a page');
        lastApp = 'browser';
      } else {
        const app = resolveAppName(target);
        if (app) {
          push('open_app', { app }, `You asked to open ${app}`);
          lastApp = app;
        }
      }
      continue;
    }

    // close
    m = clause.match(/^(?:close|quit|exit)\s+(.+)$/i);
    if (m) {
      const app = resolveAppName(unquote(m[1]));
      if (app) push('close_app', { app }, `You asked to close ${app}`);
      continue;
    }

    // focus
    m = clause.match(/^(?:focus(?:\s+on)?|switch\s+to|bring\s+(?:up\s+)?)\s*(.+)$/i);
    if (m) {
      const app = resolveAppName(unquote(m[1]));
      if (app) {
        push('focus_window', { app }, `You asked to focus ${app}`);
        lastApp = app;
      }
      continue;
    }
  }

  const v = validatePlan({ steps: raw }, 'local');
  return { summary: v.summary, steps: v.steps, notes: v.notes, planner: 'local' };
}

/**
 * Gate for Copilot routing: true when the local planner found real work, except
 * a lone theme change, which keeps using the existing theme-synthesis branch.
 */
export function isAgentIntent(prompt: string): boolean {
  const plan = localPlan(prompt);
  if (plan.steps.length === 0) return false;
  if (plan.steps.length === 1 && plan.steps[0].tool === 'set_theme') return false;
  return true;
}

// ───────────────────────── LLM planner ─────────────────────────

function buildSystemPrompt(): string {
  const tools = listTools()
    .map((t) => `- ${t.name}(${schemaSummary(t.argsSchema)}): ${t.description}`)
    .join('\n');
  return `You are the planning module of MorphOS Agent Mode. Turn the user's request into a short ordered list of tool calls.
Respond ONLY with JSON: {"summary": string, "steps": [{"tool": string, "args": object, "reason": string}]}.
Rules: use only these tools, at most ${MAX_PLAN_STEPS} steps, string args only. Plan only what the user asked for. If the request needs no tool, return an empty steps list. Use show_image after screenshot when the user wants to see it. Never use sudo or destructive commands.
Tools:
${tools}`;
}

function extractJson(text: string): unknown {
  const t = text.trim().replace(/^```(?:json)?\s*/i, '').replace(/\s*```$/, '');
  return JSON.parse(t);
}

export async function llmPlan(prompt: string, config: CloudConfig): Promise<PlanResult | null> {
  const system = buildSystemPrompt();
  const userText = `User request: ${JSON.stringify(prompt.slice(0, 1000))}`;
  try {
    const result = await runProvidersForText(config, {
      geminiBody: () => ({
        contents: [{ parts: [{ text: `${system}\n\n${userText}` }] }],
        generationConfig: { responseMimeType: 'application/json' },
      }),
      openAiMessages: [
        { role: 'system', content: system },
        { role: 'user', content: userText },
      ],
      json: true,
    });
    if (!result) return null;
    const v = validatePlan(extractJson(result.text), 'model');
    if (v.steps.length === 0) return null;
    return { summary: v.summary, steps: v.steps, notes: v.notes, planner: 'llm' };
  } catch {
    return null;
  }
}

/**
 * Creates the plan ONCE from the user's own message. The model is only used when a
 * provider is usable and the engine is not 'local'; any failure falls back to the
 * local planner. Tool output never feeds back into planning.
 */
export async function planTask(prompt: string, config: CloudConfig): Promise<PlanResult> {
  if (config.engineMode !== 'local' && hasUsableProviderForMode(config)) {
    const planned = await llmPlan(prompt, config);
    if (planned) return planned;
  }
  return localPlan(prompt);
}
