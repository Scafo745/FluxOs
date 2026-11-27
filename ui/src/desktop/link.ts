import { useEffect } from "react";

import { agentHealth, api, fluxdUrl } from "../lib/api";
import { type Subscription, subscribe } from "../lib/sse";
import type { FluxEvent } from "../lib/types";
import { useSession } from "../state/session";
import { useShell } from "../state/shell";
import { useSystem } from "../state/system";

/** Loads the system, then follows fluxd's event stream and the agent's health. */
export function useBackendLink(): void {
  useEffect(() => {
    let sub: Subscription | null = null;
    let stopped = false;
    void (async () => {
      let since = 0;
      try {
        const h = await api.health();
        since = h.event_seq;
        useSystem.setState({ bootId: h.boot_id });
      } catch {
        // The subscription below retries until fluxd answers.
      }
      await useSystem
        .getState()
        .loadAll()
        .catch(() => {});
      if (stopped) return;
      sub = subscribe({
        url: (s) => fluxdUrl(`/v1/events?since=${s}`),
        headers: (): Record<string, string> => {
          const t = useSession.getState().session?.token;
          return t ? { authorization: `Bearer ${t}` } : {};
        },
        since,
        onMessage: (m) => {
          try {
            useSystem.getState().onEvent(JSON.parse(m.data) as FluxEvent);
          } catch {
            // Ignore malformed events.
          }
        },
        onStatus: (online) => useSystem.getState().setConn(online ? "online" : "offline"),
        onAuthError: () => useSession.getState().expire(),
      });
    })();

    const pollAgent = () =>
      agentHealth()
        .then((a) => useSystem.getState().setAgent(a))
        .catch(() => useSystem.getState().setAgent("offline"));
    void pollAgent();
    const timer = setInterval(pollAgent, 15000);
    return () => {
      stopped = true;
      sub?.close();
      clearInterval(timer);
    };
  }, []);
}

/** Desktop keyboard shortcuts. */
export function useShortcuts(workspaces: number): void {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const shell = useShell.getState();
      const mod = e.ctrlKey || e.metaKey;
      if (mod && (e.key.toLowerCase() === "k" || e.code === "Space")) {
        e.preventDefault();
        shell.toggle("assistant");
        return;
      }
      if (e.key === "Escape") {
        if (shell.confirm) shell.settle();
        else if (shell.overlay) shell.show(null);
        else if (useSystem.getState().preview) useSystem.getState().endPreview();
        return;
      }
      if (e.ctrlKey && e.altKey && (e.key === "ArrowRight" || e.key === "ArrowLeft")) {
        e.preventDefault();
        const n = shell.workspace + (e.key === "ArrowRight" ? 1 : -1);
        shell.setWorkspace(((n - 1 + workspaces) % workspaces) + 1);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [workspaces]);
}
