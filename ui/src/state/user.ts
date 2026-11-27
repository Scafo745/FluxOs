// The console user's session (flux-session through fluxd): installed desktop
// apps and launching them. Optional: without flux-session the shell still
// works, and these features say why they are unavailable.

import { create } from "zustand";

import { api } from "../lib/api";
import type { DesktopApp, UserSession } from "../lib/types";
import { errorText } from "./actions";
import { useShell } from "./shell";
import { useSystem } from "./system";

interface UserState {
  info: UserSession | null;
  apps: DesktopApp[] | null;
  refresh(): Promise<void>;
  launch(app: Pick<DesktopApp, "id" | "name">): Promise<void>;
}

export const useUser = create<UserState>()((set, get) => ({
  info: null,
  apps: null,

  refresh: async () => {
    try {
      const info = await api.userSession();
      set({ info });
      if (info.available) set({ apps: await api.desktopApps() });
    } catch (e) {
      set({ info: { available: false, reason: errorText(e) } });
    }
  },

  launch: async (app) => {
    const shell = useShell.getState();
    if (get().info?.available === false) {
      shell.toast({ tone: "warn", title: `Can't start ${app.name}`, body: get().info?.reason ?? "The user session is not available." });
      return;
    }
    try {
      await api.launch(app.id);
      shell.show(null);
      const kiosk = useSystem.getState().system?.spec.desktop.environment === "fluxos";
      shell.toast({
        tone: "info",
        title: `Starting ${app.name}`,
        body: kiosk ? "Apps open full screen in the FluxOS shell. Close the app to come back here." : undefined,
      });
    } catch (e) {
      shell.toast({ tone: "danger", title: `Can't start ${app.name}`, body: errorText(e) });
    }
  },
}));
