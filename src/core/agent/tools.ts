import { useOSStore } from '../store';
import { compilePromptAsync } from '../prompt-compiler';
import { executeShell } from '../../kernel/shell';
import { checkDestructiveSignature, evaluateFileSecurity } from '../../kernel/security';
import { toPng } from 'html-to-image';
import type { AppId } from '../types';
import type { ArgsSchema, RiskTier, ToolArgs, ToolContext, ToolDef, ToolResult } from './types';

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

const sleep = (ms: number, signal?: AbortSignal) =>
  new Promise<void>((resolve) => {
    const t = window.setTimeout(resolve, ms);
    signal?.addEventListener('abort', () => { window.clearTimeout(t); resolve(); }, { once: true });
  });

const nextFrames = (n: number) =>
  new Promise<void>((resolve) => {
    const step = (left: number) => (left <= 0 ? resolve() : requestAnimationFrame(() => step(left - 1)));
    step(n);
  });

/** Waits until the browser reports the given command as finished (ready or error). */
function awaitBrowser(id: number, signal: AbortSignal, timeoutMs: number): Promise<{ state: string; summary?: string; note?: string; url: string } | null> {
  return new Promise((resolve) => {
    const finish = (v: { state: string; summary?: string; note?: string; url: string } | null) => {
      window.clearTimeout(timer);
      unsub();
      signal.removeEventListener('abort', onAbort);
      resolve(v);
    };
    const check = (st: ReturnType<typeof useOSStore.getState>['browserStatus']) => {
      if (st.commandId === id && (st.state === 'ready' || st.state === 'error')) finish(st);
    };
    const onAbort = () => finish(null);
    const timer = window.setTimeout(() => finish(null), timeoutMs);
    const unsub = useOSStore.subscribe((state) => check(state.browserStatus));
    signal.addEventListener('abort', onAbort, { once: true });
    check(useOSStore.getState().browserStatus);
  });
}

async function driveBrowser(type: 'search' | 'navigate', value: string, signal: AbortSignal): Promise<ToolResult> {
  const store = useOSStore.getState();
  store.openApp('browser');
  // Let a freshly opened Browser mount and register its effect.
  await nextFrames(2);
  const id = useOSStore.getState().issueBrowserCommand(type, value);
  const status = await awaitBrowser(id, signal, 20000);
  if (signal.aborted) return { ok: false, error: 'Cancelled.' };
  if (!status) return { ok: false, error: 'The browser did not finish loading within 20 seconds.' };
  if (status.state === 'error') return { ok: false, error: status.note || 'The browser reported an error.' };
  await nextFrames(2);
  await sleep(150, signal);
  const where = type === 'search' ? `Searched for "${value}"` : `Navigated to ${value}`;
  const preview = status.summary ? ` Answer preview (untrusted, display only): ${status.summary}` : '';
  return { ok: true, text: `${where}.${preview}`, note: status.note };
}

const pad = (n: number) => String(n).padStart(2, '0');
function stamp(): string {
  const d = new Date();
  return `${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}-${pad(d.getHours())}${pad(d.getMinutes())}${pad(d.getSeconds())}`;
}

const PNG_PREFIX = 'data:image/png;base64,';

async function captureScreenshot(a: ToolArgs, ctx: ToolContext): Promise<ToolResult> {
  const store = useOSStore.getState();
  const isDesktop = a.target === 'desktop';
  const app = a.app || 'browser';
  let node: HTMLElement | null = null;

  if (!isDesktop) {
    const win = findWindow(app);
    if (!win) return { ok: false, error: `The ${app} app is not open, so there is no window to capture.` };
    if (win.isMinimized) store.focusWindow(win.id);
  }

  const deadline = Date.now() + 5000;
  while (Date.now() < deadline && !ctx.signal.aborted) {
    if (isDesktop) {
      node = document.querySelector<HTMLElement>('[data-desktop-root]');
    } else {
      const inner = document.querySelector<HTMLElement>(`[data-window-id][data-window-app="${app}"]`);
      if (inner && !inner.querySelector('[aria-label="Loading app"]')) node = inner.parentElement || inner;
    }
    if (node) break;
    await sleep(100, ctx.signal);
  }
  if (ctx.signal.aborted) return { ok: false, error: 'Cancelled.' };
  if (!node) {
    return { ok: false, error: isDesktop ? 'The desktop is not available to capture.' : `The ${app} window did not finish loading in time.` };
  }

  await nextFrames(2);
  const root = document.querySelector<HTMLElement>('[data-desktop-root]');
  const canvas = (root && getComputedStyle(root).getPropertyValue('--canvas').trim()) || '#fafafa';
  const hadIframe = node.querySelector('iframe') !== null || node.tagName === 'IFRAME';
  const base = {
    pixelRatio: Math.min(window.devicePixelRatio || 1, 1.5),
    backgroundColor: canvas,
    // The window root is positioned with a CSS transform; reset it so the capture is not offset.
    style: {
      backdropFilter: 'none',
      WebkitBackdropFilter: 'none',
      ...(isDesktop ? {} : { transform: 'none', top: '0', left: '0' }),
    } as Partial<CSSStyleDeclaration>,
    filter: (n: Node) => !(n instanceof HTMLElement && n.tagName === 'IFRAME'),
  };
  let dataUrl: string;
  try {
    try {
      dataUrl = await toPng(node, base);
    } catch {
      dataUrl = await toPng(node, { ...base, skipFonts: true });
    }
  } catch (e) {
    return { ok: false, error: `Screenshot failed: ${e instanceof Error ? e.message : 'unknown error'}` };
  }
  if (ctx.signal.aborted) return { ok: false, error: 'Cancelled.' };
  if (!dataUrl.startsWith(PNG_PREFIX) || dataUrl.length <= 2000) {
    return { ok: false, error: 'The screenshot came out empty, so it was discarded.' };
  }

  const dirPath = '/home/developer/Pictures';
  const path = `${dirPath}/screenshot-${stamp()}.png`;
  const fresh = useOSStore.getState();
  const dir = fresh.filesystem.find((f) => f.path === dirPath && f.type === 'directory');
  let saveNote = '';
  if (!dir) {
    saveNote = 'The Pictures folder does not exist, so the image was not saved.';
  } else {
    const sec = evaluateFileSecurity(dir, 'write', fresh.profile, fresh.selinux);
    if (!sec.allowed) {
      saveNote = `Not saved to Pictures: ${sec.message || 'permission denied'}.`;
      fresh.addAuditLog(`agent.screenshot save ${path}`, 'BLOCKED', saveNote, 'WARNING');
    } else {
      fresh.writeFile(path, dataUrl);
      fresh.addAuditLog(`agent.screenshot save ${path}`, 'ALLOWED', 'Screenshot saved to Pictures', 'INFO');
      ctx.scratch.lastImagePath = path;
    }
  }
  ctx.scratch.lastImage = dataUrl;
  const what = isDesktop ? 'the desktop' : `the ${app} window`;
  const noteParts = [saveNote, hadIframe ? 'Embedded external sites (iframes) cannot be captured and appear blank.' : ''].filter(Boolean);
  return {
    ok: true,
    text: `Captured ${what}${saveNote ? '. ' + saveNote : ` and saved to ${path}`}`,
    imageDataUrl: dataUrl,
    note: noteParts.join(' ') || undefined,
  };
}

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
    execute: (a, ctx) => driveBrowser('search', a.query, ctx.signal),
  },
  {
    name: 'browser_navigate',
    description: 'Navigate the MorphOS Browser to a URL (http, https, morphos:// or a bare domain).',
    tier: 2,
    argsSchema: { url: { type: 'string', required: true, maxLen: 300, pattern: URL_PATTERN } },
    timeoutMs: 30000,
    describe: (a) => `Navigate the browser to ${a.url}`,
    execute: (a, ctx) => driveBrowser('navigate', a.url, ctx.signal),
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
    execute: captureScreenshot,
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
