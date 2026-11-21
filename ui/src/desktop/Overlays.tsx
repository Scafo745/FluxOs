import { CircleCheck, CircleX, Eye, Info, Power, ShieldExclamationPoint, TriangleExclamationPoint, X } from "lucide-react";
import { useState } from "react";

import { ArrowText, Button, ChangeList, TierBadge } from "../components/ui";
import { TIER_BEHAVIOR } from "../lib/format";
import type { Spec } from "../lib/types";
import { approvePlan, errorText, restart, restoreGeneration } from "../state/actions";
import { type Tone, useShell } from "../state/shell";
import { useSystem } from "../state/system";

const TONE: Record<Tone, { icon: typeof Info; cls: string }> = {
  info: { icon: Info, cls: "text-fg" },
  success: { icon: CircleCheck, cls: "text-ok" },
  warn: { icon: TriangleExclamationPoint, cls: "text-warn" },
  danger: { icon: CircleX, cls: "text-bad" },
  accent: { icon: Info, cls: "text-accent" },
};

export function Toasts({ spec }: { spec: Spec }) {
  const toasts = useShell((s) => s.toasts);
  const top = spec.desktop.panel.position === "top" ? spec.desktop.panel.height_px + 12 : 12;
  return (
    <div className="pointer-events-none fixed right-3 z-[60] flex w-[min(380px,calc(100vw-24px))] flex-col gap-2" style={{ top }} aria-live="polite">
      {toasts.map((t) => {
        const tone = TONE[t.tone];
        const Icon = tone.icon;
        return (
          <div key={t.id} className="glass pointer-events-auto flex gap-3 rounded-lg border hairline p-3 shadow-raised [animation:flux-toast-in_var(--flux-duration-slow)_var(--flux-easing)_both]">
            <Icon className={`mt-0.5 size-4.5 shrink-0 ${tone.cls}`} />
            <div className="min-w-0 flex-1">
              <p className="text-sm font-semibold">
                <ArrowText text={t.title} />
              </p>
              {t.body && (
                <p className="mt-0.5 line-clamp-3 text-sm text-muted">
                  <ArrowText text={t.body} />
                </p>
              )}
              {t.action && (
                <button
                  type="button"
                  onClick={() => {
                    t.action?.run();
                    useShell.getState().dismiss(t.id);
                  }}
                  className="mt-2 text-sm font-semibold text-accent hover:underline"
                >
                  {t.action.label}
                </button>
              )}
            </div>
            <button type="button" aria-label="Dismiss" onClick={() => useShell.getState().dismiss(t.id)} className="self-start rounded p-0.5 text-muted hover:text-fg">
              <X className="size-4" />
            </button>
          </div>
        );
      })}
    </div>
  );
}

/** Persistent strips for state a person should not miss. */
export function Banners({ spec }: { spec: Spec }) {
  const preview = useSystem((s) => s.preview);
  const system = useSystem((s) => s.system);
  const plans = useSystem((s) => s.plans);
  const [busy, setBusy] = useState(false);
  const top = spec.desktop.panel.position === "top" ? spec.desktop.panel.height_px + 10 : 10;
  const previewPlan = preview?.kind === "plan" ? plans.find((p) => p.id === preview.id) : undefined;
  const items: React.ReactNode[] = [];

  if (preview) {
    items.push(
      <div key="preview" className="glass glow-ring flex items-center gap-3 rounded-full py-1.5 pr-1.5 pl-4 shadow-raised">
        <Eye className="size-4 text-accent" />
        <span className="text-sm">
          <span className="font-semibold">Previewing</span> {preview.label}. Nothing has changed yet.
        </span>
        {previewPlan && (previewPlan.status === "proposed" || previewPlan.status === "approved") && (
          <Button
            size="sm"
            variant="primary"
            busy={busy}
            onClick={async () => {
              setBusy(true);
              useSystem.getState().endPreview();
              await approvePlan(previewPlan);
              setBusy(false);
            }}
          >
            Approve & apply
          </Button>
        )}
        {preview.kind === "generation" && (
          <Button
            size="sm"
            variant="primary"
            onClick={() => {
              useSystem.getState().endPreview();
              void restoreGeneration(Number(preview.id));
            }}
          >
            Restore this
          </Button>
        )}
        <Button size="sm" variant="subtle" onClick={() => useSystem.getState().endPreview()}>
          Exit preview
        </Button>
      </div>,
    );
  }
  if (!preview && system?.status === "staged") {
    items.push(
      <div key="staged" className="glass flex items-center gap-3 rounded-full py-1.5 pr-1.5 pl-4 shadow-raised">
        <Power className="size-4 text-warn" />
        <span className="text-sm">Generation {system.generation} is installed for the next boot. Kernel and boot changes start after a restart.</span>
        <Button size="sm" variant="primary" icon={Power} onClick={() => void restart()}>
          Restart now
        </Button>
      </div>,
    );
  }
  if (system?.degraded.length) {
    items.push(
      <div key="degraded" className="flex items-center gap-2 rounded-full bg-bad px-4 py-1.5 text-sm font-medium text-white shadow-raised">
        <ShieldExclamationPoint className="size-4" />
        Degraded: {system.degraded.join("; ")}. Only tier 0-1 changes are allowed.
      </div>,
    );
  }
  if (!items.length) return null;
  return (
    <div className="pointer-events-none fixed inset-x-0 z-[45] flex flex-col items-center gap-2 px-4" style={{ top }}>
      {items.map((it, i) => (
        <div key={i} className="pointer-events-auto rise-in max-w-full">
          {it}
        </div>
      ))}
    </div>
  );
}

export function ConfirmDialog() {
  const confirm = useShell((s) => s.confirm);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (!confirm) return null;
  const close = () => {
    setError(null);
    useShell.getState().settle();
  };
  return (
    <div className="fixed inset-0 z-[80] grid place-items-center bg-bg/50 p-6 fade-in" onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <div role="alertdialog" aria-modal="true" aria-label={confirm.title} className="glass w-[min(560px,100%)] rounded-xl border hairline p-6 shadow-raised win-enter">
        <div className="flex items-start gap-3">
          <ShieldExclamationPoint className="mt-0.5 size-5 shrink-0 text-warn" />
          <div className="min-w-0 flex-1">
            <h2 className="text-lg font-semibold">{confirm.title}</h2>
            {confirm.body && <p className="mt-1 text-sm text-muted">{confirm.body}</p>}
            <div className="mt-3 flex items-center gap-2">
              <TierBadge tier={confirm.tier} />
              <span className="text-xs text-muted">{TIER_BEHAVIOR[confirm.tier]}</span>
            </div>
          </div>
        </div>
        <div className="mt-4 max-h-[40vh] overflow-y-auto rounded-lg border hairline bg-bg/40 px-3 py-1">
          <ChangeList changes={confirm.changes} />
        </div>
        {error && <p className="mt-3 text-sm text-bad">{error}</p>}
        <div className="mt-5 flex justify-end gap-2">
          <Button variant="ghost" onClick={close}>
            Cancel
          </Button>
          <Button
            variant="primary"
            busy={busy}
            onClick={async () => {
              setBusy(true);
              setError(null);
              try {
                await confirm.onConfirm();
                close();
              } catch (e) {
                setError(errorText(e));
              } finally {
                setBusy(false);
              }
            }}
          >
            {confirm.confirmLabel}
          </Button>
        </div>
      </div>
    </div>
  );
}
