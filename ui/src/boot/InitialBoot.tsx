import { useEffect, useState } from "react";

import { api } from "../lib/api";
import type { BootReport } from "../lib/types";
import { useSystem } from "../state/system";
import { BootView, reducedMotion, usePaced } from "./BootView";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/**
 * First screen: waits for fluxd, themes itself with the live OS theme, then
 * replays the daemon's real boot stages (titles, details and measured
 * durations) before revealing the desktop.
 */
export function InitialBoot({ onDone }: { onDone: () => void }) {
  const [report, setReport] = useState<BootReport | null>(null);
  const [waiting, setWaiting] = useState(false);
  const [verbose, setVerbose] = useState(false);
  const [skip, setSkip] = useState(false);
  const [phase, setPhase] = useState<"waiting" | "booting" | "ready" | "exiting">("booting");

  useEffect(() => {
    let stop = false;
    void (async () => {
      for (let attempt = 0; !stop; attempt++) {
        try {
          const r = await api.boot();
          if (r.ready) {
            useSystem.setState({ bootId: r.boot_id });
            await useSystem
              .getState()
              .loadPublicTheme()
              .catch(() => {});
            if (!stop) {
              setWaiting(false);
              setReport(r);
            }
            return;
          }
        } catch {
          if (attempt > 0 && !stop) setWaiting(true);
        }
        await sleep(attempt < 3 ? 400 : 1200);
      }
    })();
    return () => {
      stop = true;
    };
  }, []);

  const stages = report?.stages ?? [];
  const fast = reducedMotion();
  const shown = usePaced(stages.length, fast ? 0 : 135, report !== null, skip);
  const allShown = report !== null && shown >= stages.length;

  useEffect(() => {
    if (!allShown) return;
    setPhase("ready");
    const hold = skip || fast ? 150 : 1100;
    const t1 = setTimeout(() => setPhase("exiting"), hold);
    const t2 = setTimeout(onDone, hold + (fast ? 50 : 640));
    return () => {
      clearTimeout(t1);
      clearTimeout(t2);
    };
  }, [allShown, skip, fast, onDone]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setVerbose((v) => !v);
      else if (e.key === " " || e.key === "Enter") setSkip(true);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const generation = report?.generation;
  const theme = useSystem((s) => /\(([^)]+)\)/.exec(s.themeCss ?? "")?.[1]);

  return (
    <BootView
      tagline="Describe the computer you need"
      stages={stages.slice(0, shown)}
      total={stages.length || 14}
      phase={waiting && !report ? "waiting" : phase}
      waitingLine="Waiting for the FluxOS core (fluxd) on this machine…"
      readyLine={
        report
          ? `Ready · fluxd booted in ${report.duration_ms} ms · generation ${generation}${theme ? ` · ${theme}` : ""}`
          : undefined
      }
      footer={[report ? `fluxd ${report.version}` : "fluxd", report ? `${report.target} target` : "connecting"]}
      verbose={verbose}
    />
  );
}
