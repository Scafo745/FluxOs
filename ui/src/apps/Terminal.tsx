import "@xterm/xterm/css/xterm.css";

import { FitAddon } from "@xterm/addon-fit";
import { Terminal as XTerm } from "@xterm/xterm";
import { RotateCcw, SquareTerminal } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { Button, Empty, Spinner } from "../components/ui";
import { api, terminalSocketUrl } from "../lib/api";
import { errorText } from "../state/actions";
import { useSystem } from "../state/system";

function cssVar(name: string, fallback: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}

/** xterm colors from the live FluxOS theme. */
function theme() {
  const bg = cssVar("--flux-color-surface", "#161322");
  const fg = cssVar("--flux-color-text", "#f1f0f5");
  const accent = cssVar("--flux-color-accent", "#7c5cff");
  return {
    background: bg,
    foreground: fg,
    cursor: accent,
    cursorAccent: bg,
    selectionBackground: `color-mix(in srgb, ${accent} 35%, transparent)`,
    blue: accent,
    brightBlue: accent,
    magenta: cssVar("--flux-color-secondary", "#f5bd3d"),
    green: cssVar("--flux-color-success", "#3ddc97"),
    yellow: cssVar("--flux-color-warning", "#f5bd3d"),
    red: cssVar("--flux-color-danger", "#ff5c7a"),
  };
}

type State = { phase: "connecting" } | { phase: "open" } | { phase: "exited"; code: number | null } | { phase: "error"; message: string };

/**
 * A real shell in the console user's session, through flux-session. It runs
 * with that user's rights: the same as a terminal on the machine itself.
 */
export function Terminal() {
  const host = useRef<HTMLDivElement>(null);
  const [state, setState] = useState<State>({ phase: "connecting" });
  const [run, setRun] = useState(0);
  const themeKey = useSystem((s) => s.themeKey);
  const term = useRef<XTerm | null>(null);

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    const t = new XTerm({
      fontFamily: cssVar("--flux-font-mono", "monospace"),
      fontSize: 13,
      lineHeight: 1.15,
      cursorBlink: true,
      allowProposedApi: false,
      theme: theme(),
      scrollback: 5000,
    });
    const fit = new FitAddon();
    t.loadAddon(fit);
    t.open(el);
    fit.fit();
    term.current = t;
    let ws: WebSocket | null = null;
    let disposed = false;
    const enc = new TextEncoder();
    const sendSize = () => {
      if (ws?.readyState === WebSocket.OPEN) ws.send(JSON.stringify({ type: "resize", cols: t.cols, rows: t.rows }));
    };
    const ro = new ResizeObserver(() => {
      try {
        fit.fit();
        sendSize();
      } catch {
        /* hidden or zero-sized */
      }
    });
    ro.observe(el);
    const input = t.onData((d) => {
      if (ws?.readyState === WebSocket.OPEN) ws.send(enc.encode(d));
    });

    setState({ phase: "connecting" });
    api
      .terminalTicket(t.cols, t.rows)
      .then(({ ticket }) => {
        if (disposed) return;
        ws = new WebSocket(terminalSocketUrl(ticket));
        ws.binaryType = "arraybuffer";
        ws.onopen = () => {
          setState({ phase: "open" });
          sendSize();
          t.focus();
        };
        ws.onmessage = (m) => {
          if (typeof m.data === "string") {
            const v = JSON.parse(m.data) as { type?: string; code?: number };
            if (v.type === "exit") setState({ phase: "exited", code: v.code ?? null });
          } else {
            t.write(new Uint8Array(m.data as ArrayBuffer));
          }
        };
        ws.onclose = () => setState((s) => (s.phase === "open" ? { phase: "exited", code: null } : s));
        ws.onerror = () => setState({ phase: "error", message: "The terminal connection failed." });
      })
      .catch((e) => !disposed && setState({ phase: "error", message: errorText(e) }));

    return () => {
      disposed = true;
      ro.disconnect();
      input.dispose();
      ws?.close();
      t.dispose();
      term.current = null;
    };
  }, [run]);

  useEffect(() => {
    if (term.current) term.current.options.theme = theme();
  }, [themeKey]);

  if (state.phase === "error") {
    return (
      <Empty icon={SquareTerminal} title="No terminal">
        {state.message}
        <div className="mt-3">
          <Button
            size="sm"
            icon={RotateCcw}
            onClick={() => {
              // Render the terminal's element again before the effect looks for it.
              setState({ phase: "connecting" });
              setRun((n) => n + 1);
            }}
          >
            Try again
          </Button>
        </div>
      </Empty>
    );
  }
  return (
    <div className="relative flex h-full flex-col" style={{ background: "var(--flux-color-surface)" }}>
      <div ref={host} className="min-h-0 flex-1 px-2 pt-2" />
      {state.phase === "connecting" && (
        <div className="absolute inset-0 grid place-items-center text-sm text-muted">
          <span className="flex items-center gap-2">
            <Spinner /> Starting your shell…
          </span>
        </div>
      )}
      {state.phase === "exited" && (
        <div className="flex items-center gap-3 border-t hairline px-3 py-2 text-sm">
          <span className="text-muted">Shell exited{state.code !== null ? ` with code ${state.code}` : ""}.</span>
          <Button size="sm" icon={RotateCcw} onClick={() => setRun((n) => n + 1)}>
            New shell
          </Button>
        </div>
      )}
    </div>
  );
}
