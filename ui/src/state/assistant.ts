// Conversation with the FluxOS AI. Each turn streams the orchestrator's
// pipeline steps (route, model, policy check, health checks) as they happen.

import { create } from "zustand";

import { streamIntent } from "../lib/api";
import type { IntentResult, PlanRecord, TraceStep } from "../lib/types";
import { errorText } from "./actions";
import { clientId } from "./session";

export interface Turn {
  id: string;
  prompt: string;
  answers: Record<string, string>;
  steps: TraceStep[];
  result: IntentResult | null;
  error: string | null;
  running: boolean;
  /** Approval or rejection done from the result card. */
  followUp: { state: "working" | "done" | "error"; plan?: PlanRecord; message?: string } | null;
}

interface AssistantState {
  turns: Turn[];
  draft: string;
  history: string[];
  setDraft(s: string): void;
  ask(prompt: string, answers?: Record<string, string>): Promise<void>;
  setFollowUp(turnId: string, f: Turn["followUp"]): void;
  clear(): void;
}

let seq = 0;

export const useAssistant = create<AssistantState>()((set, get) => ({
  turns: [],
  draft: "",
  history: [],

  setDraft: (draft) => set({ draft }),

  ask: async (prompt, answers = {}) => {
    const text = prompt.trim();
    if (!text || get().turns.some((t) => t.running)) return;
    const id = `turn${++seq}`;
    const turn: Turn = { id, prompt: text, answers, steps: [], result: null, error: null, running: true, followUp: null };
    set((s) => ({
      turns: [...s.turns.slice(-11), turn],
      draft: "",
      history: Object.keys(answers).length ? s.history : [...s.history.filter((h) => h !== text), text].slice(-20),
    }));
    const patch = (p: Partial<Turn>) => set((s) => ({ turns: s.turns.map((t) => (t.id === id ? { ...t, ...p } : t)) }));
    try {
      // The conversation uses this tab's id, so the plans it creates are recognizably ours.
      const result = await streamIntent({ prompt: text, session_id: clientId, answers }, (step) =>
        set((s) => ({ turns: s.turns.map((t) => (t.id === id ? { ...t, steps: [...t.steps, step] } : t)) })),
      );
      patch({ result, running: false });
    } catch (e) {
      patch({ error: errorText(e), running: false });
    }
  },

  setFollowUp: (turnId, followUp) => set((s) => ({ turns: s.turns.map((t) => (t.id === turnId ? { ...t, followUp } : t)) })),

  clear: () => set({ turns: [] }),
}));
