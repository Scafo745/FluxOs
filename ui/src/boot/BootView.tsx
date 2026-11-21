import { Check, TriangleExclamationPoint, X } from "lucide-react";
import { useEffect, useState } from "react";

import { FluxMark } from "../components/FluxMark";
import { ArrowText, Kbd } from "../components/ui";
import type { BootStage } from "../lib/types";

export function reducedMotion(): boolean {
  return (
    document.documentElement.dataset.motion === "reduced" ||
    window.matchMedia?.("(prefers-reduced-motion: reduce)").matches === true
  );
}

/** Reveals `total` items one at a time, `stepMs` apart. */
export function usePaced(total: number, stepMs: number, active: boolean, skip: boolean): number {
  const [n, setN] = useState(0);
  useEffect(() => {
    if (!active) return;
    if (skip || stepMs <= 0) {
      setN(total);
      return;
    }
    if (n >= total) return;
    const t = setTimeout(() => setN((v) => v + 1), n === 0 ? stepMs * 3 : stepMs);
    return () => clearTimeout(t);
  }, [n, total, active, stepMs, skip]);
  return Math.min(n, total);
}

function ms(v: number): string {
  if (v <= 0) return "";
  return v < 1 ? "<1 ms" : `${v} ms`;
}

function Glyph({ status }: { status: BootStage["status"] }) {
  if (status === "fail") return <X className="size-3.5 text-bad" strokeWidth={3} />;
  if (status === "warn") return <TriangleExclamationPoint className="size-3.5 text-warn" strokeWidth={2.5} />;
  return <Check className="size-3.5 text-ok" strokeWidth={3} />;
}

export function Aurora({ intensity = 1 }: { intensity?: number }) {
  return (
    <div className="pointer-events-none absolute inset-0 overflow-hidden" aria-hidden="true">
      <div
        className="absolute top-[-20%] left-[-10%] h-[70vmax] w-[70vmax] rounded-full blur-[110px] [animation:flux-drift-a_22s_ease-in-out_infinite]"
        style={{ background: "radial-gradient(circle, var(--flux-color-accent), transparent 62%)", opacity: 0.38 * intensity }}
      />
      <div
        className="absolute right-[-15%] bottom-[-25%] h-[65vmax] w-[65vmax] rounded-full blur-[120px] [animation:flux-drift-b_26s_ease-in-out_infinite]"
        style={{ background: "radial-gradient(circle, var(--flux-color-secondary), transparent 60%)", opacity: 0.26 * intensity }}
      />
      <div
        className="absolute top-[30%] left-[35%] h-[40vmax] w-[40vmax] rounded-full blur-[100px] [animation:flux-drift-c_19s_ease-in-out_infinite]"
        style={{ background: "radial-gradient(circle, color-mix(in oklab, var(--flux-color-accent) 60%, var(--flux-color-success)), transparent 60%)", opacity: 0.18 * intensity }}
      />
    </div>
  );
}

export interface BootViewProps {
  title?: string;
  tagline: string;
  stages: BootStage[];
  /** Stages expected in total, for the progress ring. */
  total: number;
  phase: "waiting" | "booting" | "ready" | "exiting";
  readyLine?: string;
  waitingLine?: string;
  footer: string[];
  verbose: boolean;
}

export function BootView({ title = "FluxOS", tagline, stages, total, phase, readyLine, waitingLine, footer, verbose }: BootViewProps) {
  const done = stages.length;
  const progress = phase === "ready" || phase === "exiting" ? 1 : total ? done / total : 0;
  const lines = verbose ? stages : stages.slice(-7);
  const worst = stages.some((s) => s.status === "fail") ? "fail" : stages.some((s) => s.status === "warn") ? "warn" : "ok";

  return (
    <div
      className={`fixed inset-0 z-[100] overflow-hidden bg-bg text-fg ${phase === "exiting" ? "[animation:flux-exit_650ms_var(--flux-easing)_forwards]" : ""}`}
      role="status"
      aria-live="polite"
      aria-label={`${title} is starting`}
    >
      <Aurora intensity={phase === "waiting" ? 0.6 : 1} />
      <div className="grain absolute inset-0" />
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_center,transparent_25%,var(--flux-color-background)_88%)]" />

      <div className="relative flex h-full flex-col items-center justify-center px-6">
        <div className={`relative ${phase === "ready" ? "[animation:flux-pulse_1.2s_ease-in-out_1]" : ""}`}>
          <div
            className="absolute inset-[-30%] rounded-full blur-2xl"
            style={{ background: "radial-gradient(circle, color-mix(in oklab, var(--flux-color-accent) 45%, transparent), transparent 70%)", opacity: 0.7 }}
          />
          <FluxMark size={124} progress={progress} spinning={phase !== "ready" && phase !== "exiting"} glow draw className="relative" />
        </div>

        <h1 className="text-gradient mt-9 font-display text-[clamp(46px,7.5vw,84px)] leading-none font-bold tracking-[0.02em]">{title}</h1>
        <p className="mt-4 text-sm tracking-[0.34em] text-muted uppercase">{tagline}</p>

        <div className="mt-11 w-[min(620px,88vw)]">
          <div className="relative h-[3px] overflow-hidden rounded-full bg-fg/10">
            <div
              className="absolute inset-y-0 left-0 rounded-full bg-linear-to-r from-accent to-secondary transition-[width] duration-500"
              style={{ width: `${Math.round(progress * 100)}%` }}
            />
            {phase !== "ready" && phase !== "exiting" && (
              <div className="absolute inset-y-0 w-1/4 bg-linear-to-r from-transparent via-fg/40 to-transparent [animation:flux-scan_1.8s_linear_infinite]" />
            )}
          </div>

          <ol className={`mt-5 font-mono text-[13px] leading-6 ${verbose ? "max-h-[38vh] overflow-y-auto pr-2" : ""}`} aria-label="Boot log">
            {lines.map((s, i) => (
              <li
                key={`${s.id}-${i}`}
                className="rise-in grid grid-cols-[16px_minmax(110px,170px)_1fr_auto] items-center gap-3"
                style={{ opacity: verbose || i >= lines.length - 5 ? 1 : 0.42 + i * 0.12 }}
              >
                <Glyph status={s.status} />
                <span className="truncate text-fg">{s.title}</span>
                <span className="truncate text-muted" title={s.detail}>
                  <ArrowText text={s.detail} />
                </span>
                <span className="text-right text-muted/70 tabular-nums">{ms(s.duration_ms)}</span>
              </li>
            ))}
            {phase === "waiting" && waitingLine && (
              <li className="flex items-center gap-3 text-muted">
                <span className="size-2 rounded-full bg-warn [animation:flux-breathe_1.4s_ease-in-out_infinite]" />
                {waitingLine}
              </li>
            )}
          </ol>

          <div className="mt-4 h-6 text-center text-sm">
            {(phase === "ready" || phase === "exiting") && readyLine && (
              <span className={`rise-in font-medium ${worst === "fail" ? "text-bad" : worst === "warn" ? "text-warn" : "text-ok"}`}>{readyLine}</span>
            )}
          </div>
        </div>
      </div>

      <footer className="absolute inset-x-0 bottom-6 flex flex-wrap items-center justify-center gap-x-6 gap-y-2 px-6 text-xs text-muted">
        {footer.map((f) => (
          <span key={f}>{f}</span>
        ))}
        <span className="flex items-center gap-1.5">
          <Kbd>Esc</Kbd> {verbose ? "hide details" : "details"}
        </span>
        <span className="flex items-center gap-1.5">
          <Kbd>Space</Kbd> skip
        </span>
      </footer>
    </div>
  );
}
