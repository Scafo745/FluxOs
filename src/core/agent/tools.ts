import { useOSStore } from '../store';
import { compilePromptAsync } from '../prompt-compiler';
import { executeShell } from '../../kernel/shell';
import { checkDestructiveSignature, evaluateFileSecurity } from '../../kernel/security';
import type { AppId } from '../types';
import type { ArgsSchema, RiskTier, ToolArgs, ToolDef, ToolResult } from './types';

/**
 * Tool registry. Default deny: only tools registered here can ever be planned
 * or executed. Every tool re-validates its args and goes through the same
 * guards as the Terminal (executeShell, evaluateFileSecurity).
 */

export const AGENT_APPS = ['browser', 'terminal', 'filemanager', 'settings', 'security', 'copilot', 'payments'] as const;
export type AgentAppName = (typeof AGENT_APPS)[number];

const APP_ALIASES: Record<AgentAppName, string[]> = {
  browser: ['browser', 'web browser', 'internet', 'safari', 'chrome', 'the web'],
  terminal: ['terminal', 'shell', 'console', 'command line', 'cli', 'term'],
  filemanager: ['file manager', 'filemanager', 'files', 'file explorer', 'explorer', 'finder', 'file browser'],
  settings: ['settings', 'system settings', 'preferences', 'synthesizer', 'os synthesizer'],
  security: ['security', 'security guard', 'security app', 'compliance', 'security and compliance'],
  copilot: ['copilot', 'morph copilot', 'assistant', 'chat'],
  payments: ['payments', 'payment', 'payments console', 'payment console', 'payments app'],
};

/** Resolves a loose app phrase ("the web browser", "terminal app") to an app id, or null. */
export function resolveAppName(text: string): AgentAppName | null {
  let t = text.toLowerCase().trim().replace(/[.!?]+$/, '');
  t = t.replace(/^(?:the|a|an|my)\s+/, '');
  t = t.replace(/\s+(?:app|application|window|tab|program)$/, '');
  t = t.replace(/^(?:the|a|an|my)\s+/, '');
  for (const app of AGENT_APPS) {
    if (APP_ALIASES[app].includes(t)) return app;
  }
  return null;
}

const SYSTEM_PACKAGES = ['firefox', 'chromium', 'chrome', 'tor', 'git', 'curl', 'wget', 'nodejs', 'python3', 'vim', 'htop', 'browser'];
const SECURITY_WORDS = /security|zero[- ]?trust|pci|compliance|strict|selinux|sudo/i;
const SHELL_SPLIT = /\|\||&&|;|\||`|\$\(/;

const URL_PATTERN = /^(?:https?:\/\/[^\s]+|morphos:\/\/[^\s]*|[a-z0-9-]+(?:\.[a-z0-9-]+)+(?:[/?#]\S*)?)$/i;
const PATH_PATTERN = /^[A-Za-z0-9_./-]+$/;
const PKG_PATTERN = /^[A-Za-z0-9@][A-Za-z0-9@._/-]*$/;

function shellBlockReason(command: string): string | undefined {
  if (/\bsudo\b/i.test(command)) {
    return 'The agent never runs commands with sudo. Run it yourself in the Terminal if you really want it.';
  }
  const stages = [command, ...command.split(SHELL_SPLIT)];
  for (const stage of stages) {
    const d = checkDestructiveSignature(stage);
    if (d.isDestructive) return d.reason || 'Blocked by the destructive command guard.';
  }
  return undefined;
}

function shellTier(args: ToolArgs): RiskTier {
  const cmd = args.command || '';
  if (shellBlockReason(cmd)) return 4;
  if (/(^|\s)(?:rm|mv|chmod|chown|kill|killall|setenforce|mkdir)\b/i.test(cmd) || />/.test(cmd)) return 3;
  return 2;
}

export function installManager(name: string, manager?: string): string {
  if (manager) return manager;
  return SYSTEM_PACKAGES.includes(name.toLowerCase()) ? 'apt' : 'npm';
}

function findWindow(app: string) {
  return useOSStore.getState().windows.find((w) => w.appId === app);
}

const appField = { type: 'enum', required: true, values: AGENT_APPS } as const;

/** Placeholder for tools whose runtime (browser bridge, DOM capture) is attached separately. */
const notAvailable = async (): Promise<ToolResult> => ({
  ok: false,
  error: 'This tool is not available in this build.',
});

export const TOOLS: ToolDef[] = [
  {
    name: 'open_app',
    description: 'Open an app window (or bring it to the front).',
    tier: 1,
    argsSchema: { app: appField },
    timeoutMs: 5000,
    describe: (a) => `Open the ${a.app} app`,
    execute: async (a) => {
      useOSStore.getState().openApp(a.app as AppId);
      return { ok: true, text: `Opened ${a.app}.` };
    },
  },
  {
    name: 'close_app',
    description: 'Close an open app window.',
    tier: 1,
    argsSchema: { app: appField },
    timeoutMs: 5000,
    describe: (a) => `Close the ${a.app} window`,
    execute: async (a) => {
      const win = findWindow(a.app);
      if (!win) return { ok: false, error: `The ${a.app} app is not open.` };
      useOSStore.getState().closeWindow(win.id);
      return { ok: true, text: `Closed ${a.app}.` };
    },
  },
  {
    name: 'focus_window',
    description: 'Bring an open app window to the front.',
    tier: 1,
    argsSchema: { app: appField },
    timeoutMs: 5000,
    describe: (a) => `Bring the ${a.app} window to the front`,
    execute: async (a) => {
      const win = findWindow(a.app);
      if (!win) return { ok: false, error: `The ${a.app} app is not open.` };
      useOSStore.getState().focusWindow(win.id);
      return { ok: true, text: `Focused ${a.app}.` };
    },
  },
  {
    name: 'browser_search',
    description: 'Search in the MorphOS Browser and wait for the results page.',
    tier: 2,
    argsSchema: { query: { type: 'string', required: true, maxLen: 200 } },
    timeoutMs: 30000,
    describe: (a) => `Search the browser for "${a.query}"`,
    execute: notAvailable,
  },
  {
    name: 'browser_navigate',
    description: 'Navigate the MorphOS Browser to a URL (http, https, morphos:// or a bare domain).',
    tier: 2,
    argsSchema: { url: { type: 'string', required: true, maxLen: 300, pattern: URL_PATTERN } },
    timeoutMs: 30000,
    describe: (a) => `Navigate the browser to ${a.url}`,
    execute: notAvailable,
  },
  {
    name: 'screenshot',
    description: 'Capture a PNG screenshot of an app window or the whole desktop.',
    tier: 1,
    argsSchema: {
      target: { type: 'enum', required: true, values: ['window', 'desktop'] },
      app: { type: 'enum', values: AGENT_APPS },
    },
    timeoutMs: 30000,
    describe: (a) => (a.target === 'desktop' ? 'Capture the whole desktop' : `Capture the ${a.app || 'browser'} window`),
    execute: notAvailable,
  },
  {
    name: 'show_image',
    description: 'Show the most recent screenshot, or a PNG file from the file system.',
    tier: 0,
    argsSchema: { source: { type: 'string', maxLen: 200, pattern: PATH_PATTERN } },
    timeoutMs: 5000,
    describe: (a) => (!a.source || a.source === 'last' ? 'Show the latest screenshot' : `Show the image ${a.source}`),
    execute: async (a, ctx) => {
      const src = a.source || 'last';
      if (src === 'last') {
        const last = ctx.scratch.lastImage;
        if (typeof last !== 'string') return { ok: false, error: 'There is no screenshot to show yet.' };
        return { ok: true, text: 'Showing the latest screenshot.', imageDataUrl: last };
      }
      const { filesystem, profile, selinux, currentPath } = useOSStore.getState();
      const path = src.startsWith('/') ? src : `${currentPath === '/' ? '' : currentPath}/${src}`;
      const file = filesystem.find((f) => f.path === path && f.type === 'file');
      if (!file) return { ok: false, error: `${src}: No such file.` };
      const sec = evaluateFileSecurity(file, 'read', profile, selinux);
      if (!sec.allowed) return { ok: false, error: sec.message || 'Permission denied.' };
      if (!file.content || !file.content.startsWith('data:image/png;base64,')) {
        return { ok: false, error: `${src} is not a PNG image saved by MorphOS.` };
      }
      return { ok: true, text: `Showing ${path}.`, imageDataUrl: file.content };
    },
  },
  {
    name: 'run_shell',
    description: 'Run one command in the MorphOS shell. Destructive commands and sudo are blocked.',
    tier: shellTier,
    blockReason: (a) => shellBlockReason(a.command || ''),
    argsSchema: { command: { type: 'string', required: true, maxLen: 200 } },
    timeoutMs: 15000,
    describe: (a) => `Run in the shell: ${a.command}`,
    execute: async (a) => {
      // Defense in depth: the planner and runner already refuse tier 4.
      if (shellBlockReason(a.command)) return { ok: false, error: 'Blocked command.' };
      const res = executeShell(a.command);
      return res.isError ? { ok: false, error: res.output || 'Command failed.' } : { ok: true, text: res.output || '(no output)' };
    },
  },
  {
    name: 'install_package',
    description: 'Install a package (npm, apt or pip) through the MorphOS package manager.',
    tier: 2,
    argsSchema: {
      name: { type: 'string', required: true, maxLen: 80, pattern: PKG_PATTERN },
      manager: { type: 'enum', values: ['npm', 'apt', 'pip'] },
    },
    timeoutMs: 20000,
    describe: (a) => `Install ${a.name} with ${installManager(a.name, a.manager)}`,
    execute: async (a) => {
      const res = executeShell(`${installManager(a.name, a.manager)} install ${a.name}`);
      return res.isError ? { ok: false, error: res.output || 'Install failed.' } : { ok: true, text: res.output || 'Installed.' };
    },
  },
  {
    name: 'set_theme',
    description: 'Change the OS look and feel from a short description.',
    tier: (a) => (SECURITY_WORDS.test(a.prompt || '') ? 3 : 0),
    argsSchema: { prompt: { type: 'string', required: true, maxLen: 300 } },
    timeoutMs: 25000,
    describe: (a) => `Change the OS theme: "${a.prompt}"`,
    execute: async (a, ctx) => {
      const store = useOSStore.getState();
      const { profile, source } = await compilePromptAsync(a.prompt, store.cloudConfig);
      if (ctx.signal.aborted) return { ok: false, error: 'Cancelled.' };
      store.setProfile(profile);
      return { ok: true, text: `Applied theme "${profile.name}" (${source} engine).` };
    },
  },
  {
    name: 'read_file',
    description: 'Read a text file from the MorphOS file system (read only, subject to file permissions).',
    tier: 1,
    argsSchema: { path: { type: 'string', required: true, maxLen: 200, pattern: PATH_PATTERN } },
    timeoutMs: 5000,
    describe: (a) => `Read the file ${a.path}`,
    execute: async (a) => {
      if (a.path.startsWith('-')) return { ok: false, error: 'Invalid path.' };
      // Goes through the shell's cat so evaluateFileSecurity and the audit log still apply.
      const res = executeShell(`cat ${a.path}`);
      return res.isError ? { ok: false, error: res.output || 'Could not read file.' } : { ok: true, text: res.output || '(empty file)' };
    },
  },
];

const REGISTRY = new Map<string, ToolDef>(TOOLS.map((t) => [t.name, t]));

export function getTool(name: string): ToolDef | undefined {
  return REGISTRY.has(name) ? REGISTRY.get(name) : undefined;
}

export function listTools(): ToolDef[] {
  return TOOLS;
}

export function tierFor(tool: ToolDef, args: ToolArgs): RiskTier {
  return typeof tool.tier === 'function' ? tool.tier(args) : tool.tier;
}

export function schemaSummary(schema: ArgsSchema): string {
  return Object.entries(schema)
    .map(([k, s]) => `${k}${s.required ? '' : '?'}${s.type === 'enum' ? `: ${(s.values || []).join('|')}` : ''}`)
    .join(', ');
}
