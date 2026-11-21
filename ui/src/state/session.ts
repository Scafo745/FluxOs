import { create } from "zustand";
import { persist } from "zustand/middleware";

import type { Mode, Principal } from "../lib/types";

/** A paired screen. The token is a revocable fluxd session, not a master key. */
export interface Session {
  token: string;
  intentToken: string;
  principal: Principal;
  sessionId: string;
  name: string;
  mode: Mode;
  agentUrl: string;
  pairedAt: string;
}

interface SessionState {
  session: Session | null;
  /** fluxd rejected the token (revoked, or its state was reset). */
  expired: boolean;
  set(session: Session): void;
  expire(): void;
  clear(): void;
}

export const useSession = create<SessionState>()(
  persist(
    (set) => ({
      session: null,
      expired: false,
      set: (session) => set({ session, expired: false }),
      expire: () => set((s) => (s.session ? { session: null, expired: true } : s)),
      clear: () => set({ session: null, expired: false }),
    }),
    { name: "fluxos.session.v1", partialize: (s) => ({ session: s.session }) },
  ),
);

/**
 * Id of this browser tab. Plans this UI starts (directly or through the AI)
 * carry it, so the UI can tell its own changes from other clients'.
 */
export const clientId: string = (() => {
  const key = "fluxos.client";
  let id = sessionStorage.getItem(key);
  if (!id) {
    id = `ui-${crypto.randomUUID().replace(/-/g, "").slice(0, 16)}`;
    sessionStorage.setItem(key, id);
  }
  return id;
})();
