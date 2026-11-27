import { useEffect, useState } from "react";

import { useSystem } from "../state/system";
import { BootView, reducedMotion, usePaced } from "./BootView";

/**
 * Plays a restart: a simulated reboot of the sandbox target (new kernel,
 * scheduler and session, real health checks) or fluxd itself restarting.
 */
export function RestartOverlay() {
  const restart = useSystem((s) => s.restart);
  const theme = useSystem((s) => s.system?.theme_name);
  const [verbose, setVerbose] = useState(false);
  const [skip, setSkip] = useState(false);
  const [phase, setPhase] = useState<"booting" | "ready" | "exiting">("booting");
  const stages = restart?.stages ?? [];
  const fast = reducedMotion();
  const shown = usePaced(stages.length, fast ? 0 : 320, restart !== null, skip);
  const finished = restart?.ready === true && shown >= stages.length;

  useEffect(() => {
    if (!restart) {
      setPhase("booting");
      setSkip(false);
      setVerbose(false);
    }
  }, [restart]);

  useEffect(() => {
    if (!finished) return;
    setPhase("ready");
    const hold = skip || fast ? 150 : 1000;
    const t1 = setTimeout(() => setPhase("exiting"), hold);
    const t2 = setTimeout(() => useSystem.getState().endRestart(), hold + (fast ? 50 : 640));
    return () => {
      clearTimeout(t1);
      clearTimeout(t2);
    };
  }, [finished, skip, fast]);

  useEffect(() => {
    if (!restart) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setVerbose((v) => !v);
      else if (e.key === " ") setSkip(true);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [restart]);

  if (!restart) return null;
  const reboot = restart.kind === "reboot";
  return (
    <BootView
      title="FluxOS"
      tagline={reboot ? "Restarting into the new generation" : "FluxOS core restarted"}
      stages={stages.slice(0, shown)}
      total={reboot ? 6 : Math.max(stages.length + 1, 14)}
      phase={phase}
      readyLine={`Ready · generation ${restart.generation ?? "?"}${theme ? ` · ${theme}` : ""}`}
      footer={[reboot ? "simulated reboot (sandbox target)" : "reconnected to fluxd"]}
      verbose={verbose}
    />
  );
}
