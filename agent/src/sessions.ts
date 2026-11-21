// In-memory conversation memory and a small plan cache (PRD: caching to
// keep inference cost down). Nothing here is security relevant: fluxd
// re-validates every plan against the live state.

import type { Draft, IntentStatus, Question } from "./types.js";

export interface Turn {
  at: string;
  prompt: string;
  status: IntentStatus;
  outcome: string;
  plan_id?: string;
  generation?: number | null;
}

export interface Session {
  id: string;
  created_at: string;
  turns: Turn[];
  pending?: { prompt: string; questions: Question[] };
}

const MAX_SESSIONS = 200;
const MAX_TURNS = 20;

export class Sessions {
  private readonly map = new Map<string, Session>();

  get(id: string | undefined): Session {
    const key = id && /^[A-Za-z0-9_-]{1,64}$/.test(id) ? id : `ss_${crypto.randomUUID().replace(/-/g, "").slice(0, 20)}`;
    let s = this.map.get(key);
    if (!s) {
      s = { id: key, created_at: new Date().toISOString(), turns: [] };
      this.map.set(key, s);
      if (this.map.size > MAX_SESSIONS) this.map.delete(this.map.keys().next().value!);
    }
    return s;
  }

  peek(id: string): Session | undefined {
    return this.map.get(id);
  }

  record(s: Session, t: Turn) {
    s.turns.push(t);
    if (s.turns.length > MAX_TURNS) s.turns.splice(0, s.turns.length - MAX_TURNS);
  }

  history(s: Session, n = 4): { prompt: string; outcome: string }[] {
    return s.turns.slice(-n).map((t) => ({ prompt: t.prompt, outcome: t.outcome }));
  }
}

export class PlanCache {
  private readonly map = new Map<string, { draft: Draft; engine: { provider?: string; model?: string } }>();

  constructor(private readonly max = 100) {}

  static key(prompt: string, answers: Record<string, string>, specDigest: string | undefined): string {
    const norm = prompt.toLowerCase().replace(/[^\p{L}\p{N}#]+/gu, " ").trim();
    return JSON.stringify([norm, Object.entries(answers).sort(), specDigest ?? ""]);
  }

  get(k: string) {
    const v = this.map.get(k);
    if (v) {
      this.map.delete(k);
      this.map.set(k, v);
    }
    return v;
  }

  set(k: string, v: { draft: Draft; engine: { provider?: string; model?: string } }) {
    this.map.set(k, v);
    if (this.map.size > this.max) this.map.delete(this.map.keys().next().value!);
  }
}
