// User actions shared by the assistant, apps, toasts and banners. Each one
// goes through fluxd, which re-checks permissions and policy on every call.

import { ApiError, api, restoreImpact } from "../lib/api";
import type { PlanRecord } from "../lib/types";
import { clientId } from "./session";
import { useShell } from "./shell";
import { useSystem } from "./system";

export function errorText(e: unknown): string {
  if (e instanceof ApiError) return e.message;
  if (e instanceof Error) return e.message;
  return String(e);
}

/** Toast describing the outcome of a plan this UI started. */
export function reportPlan(plan: PlanRecord, quietWhenApplied = false): void {
  const shell = useShell.getState();
  const gen = plan.result?.generation;
  const first = plan.change_summary.split("\n")[0] ?? "";
  switch (plan.status) {
    case "applied":
      if (!quietWhenApplied) shell.toast({ tone: "success", title: `Generation ${gen} is live`, body: first });
      break;
    case "staged":
      shell.toast({
        tone: "warn",
        title: `Generation ${gen} staged for restart`,
        body: "Kernel and boot changes take effect after a restart. The previous version stays in the boot menu.",
        action: { label: "Restart now", run: () => void restart() },
        ttl: 14000,
      });
      break;
    case "proposed":
    case "approved":
      if (plan.decision !== "auto_apply") {
        shell.toast({
          tone: "warn",
          title: `Needs approval · tier ${plan.evaluation.tier}`,
          body: first,
          action: { label: "Review", run: () => useShell.getState().open("approvals", { planId: plan.id }) },
        });
      }
      break;
    case "rolled_back":
    case "failed":
      shell.toast({ tone: "danger", title: plan.status === "failed" ? "Apply failed" : "Rolled back", body: plan.result?.message, ttl: 12000 });
      break;
    case "blocked":
    case "invalid": {
      const why = plan.evaluation.issues.filter((i) => i.severity === "error").map((i) => i.message).join(" ");
      shell.toast({ tone: "danger", title: plan.status === "blocked" ? "Refused by policy" : "Not a valid change", body: why, ttl: 10000 });
      break;
    }
    case "no_op":
      shell.toast({ tone: "info", title: "Already set", body: "The system already looks like that." });
      break;
    default:
      break;
  }
}

/** Approve with the hash of the plan the person reviewed, then apply. */
export async function approvePlan(plan: PlanRecord): Promise<PlanRecord | null> {
  try {
    const result = await api.approve(plan.id, plan.plan_hash);
    reportPlan(result);
    void useSystem.getState().refreshPlans();
    return result;
  } catch (e) {
    useShell.getState().toast({ tone: "danger", title: "Could not approve", body: errorText(e), ttl: 10000 });
    return null;
  }
}

export async function rejectPlan(id: string): Promise<void> {
  try {
    await api.reject(id, "Rejected in the FluxOS UI");
    void useSystem.getState().refreshPlans();
  } catch (e) {
    useShell.getState().toast({ tone: "danger", title: "Could not reject", body: errorText(e) });
  }
}

/**
 * Restores a generation. fluxd computes the risk of going back; tier 2+
 * restores show the exact changes and need an explicit confirmation.
 */
export async function restoreGeneration(to: number): Promise<void> {
  const shell = useShell.getState();
  try {
    const impact = await restoreImpact(to, clientId);
    if (!impact.ok) {
      shell.toast({ tone: "danger", title: `Can't restore generation ${to}`, body: impact.error });
      return;
    }
    if (impact.changes.length === 0) {
      shell.toast({ tone: "info", title: "Nothing to restore", body: `The system already matches generation ${to}.` });
      return;
    }
    const run = async () => {
      const plan = await api.rollback(to, `Restore generation ${to}`, clientId);
      reportPlan(plan);
    };
    if (impact.tier >= 2) {
      shell.ask({
        title: `Restore generation ${to}?`,
        body: "Going back changes system settings, so FluxOS asks first. The current generation stays available.",
        tier: impact.tier,
        changes: impact.changes,
        confirmLabel: "Restore",
        onConfirm: run,
      });
    } else {
      await run();
    }
  } catch (e) {
    shell.toast({ tone: "danger", title: "Restore failed", body: errorText(e) });
  }
}

/** Undo the change that produced `generation` by restoring its parent. */
export async function undoGeneration(generation: number): Promise<void> {
  try {
    const { meta } = await api.generation(generation);
    if (meta.parent === null) {
      useShell.getState().toast({ tone: "info", title: "Nothing to undo", body: "This is the first generation." });
      return;
    }
    await restoreGeneration(meta.parent);
  } catch (e) {
    useShell.getState().toast({ tone: "danger", title: "Undo failed", body: errorText(e) });
  }
}

export async function restart(): Promise<void> {
  try {
    await api.reboot();
  } catch (e) {
    useShell.getState().toast({ tone: "danger", title: "Restart failed", body: errorText(e) });
  }
}

/** Restart or shut down after a confirmation. */
export function power(kind: "restart" | "shutdown"): void {
  const shell = useShell.getState();
  shell.show(null);
  shell.ask({
    title: kind === "restart" ? "Restart FluxOS?" : "Shut down FluxOS?",
    body:
      kind === "restart"
        ? "Open apps close. The current generation starts again; a staged generation becomes active."
        : "Open apps close and the machine turns off.",
    changes: [],
    confirmLabel: kind === "restart" ? "Restart" : "Shut down",
    onConfirm: async () => {
      if (kind === "restart") await api.reboot();
      else {
        await api.poweroff();
        shell.toast({ tone: "info", title: "Shutting down", body: "FluxOS is turning the machine off.", ttl: 0 });
      }
    },
  });
}

/** Tier 0-1 tweak from quick settings; the theme change itself is the feedback. */
export async function quick(label: string, ops: PlanRecord["operations"]): Promise<void> {
  try {
    const plan = await useSystem.getState().proposeUi(label, ops, true);
    reportPlan(plan, true);
  } catch (e) {
    useShell.getState().toast({ tone: "danger", title: "Change failed", body: errorText(e) });
  }
}
