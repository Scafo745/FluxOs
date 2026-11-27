import { useEffect, useState } from "react";

import { Assistant } from "../assistant/Assistant";
import { RestartOverlay } from "../boot/RestartOverlay";
import { FluxMark } from "../components/FluxMark";
import { useShell } from "../state/shell";
import { selectSpec, useSystem } from "../state/system";
import { useUser } from "../state/user";
import { ControlCenter } from "./ControlCenter";
import { DesktopIcons } from "./DesktopIcons";
import { Dock } from "./Dock";
import { Launcher } from "./Launcher";
import { useBackendLink, useShortcuts } from "./link";
import { Banners, ConfirmDialog, Toasts } from "./Overlays";
import { Panel } from "./Panel";
import { useViewport, workArea } from "./layout";
import { Wallpaper } from "./Wallpaper";
import { SnapPreview, WindowLayer } from "./WindowLayer";

function Connecting() {
  const [late, setLate] = useState(false);
  useEffect(() => {
    const t = setTimeout(() => setLate(true), 400);
    return () => clearTimeout(t);
  }, []);
  return (
    <div className="fixed inset-0 bg-bg">
      <Wallpaper />
      {late && (
        <div className="absolute inset-0 grid place-items-center fade-in">
          <div className="flex items-center gap-3 rounded-full bg-bg/60 px-5 py-2.5 text-sm text-muted">
            <FluxMark size={22} spinning /> Loading your desktop…
          </div>
        </div>
      )}
    </div>
  );
}

/**
 * The FluxOS web desktop. Its layout is the OS spec: dock position and size,
 * panel, tiling, gaps, workspaces, fonts, colors and motion all come from the
 * active generation (or the plan being previewed).
 */
export function Desktop() {
  const spec = useSystem(selectSpec);
  const ready = useSystem((s) => s.system !== null);
  useBackendLink();
  useShortcuts(spec?.desktop.workspaces ?? 1);
  const vp = useViewport();

  useEffect(() => {
    if (spec) useShell.getState().setArea(workArea(spec, vp.w, vp.h));
  }, [spec, vp.w, vp.h]);

  // The user's desktop apps, so dock icons can start them.
  useEffect(() => {
    if (ready) void useUser.getState().refresh();
  }, [ready]);

  useEffect(() => {
    if (!ready || !sessionStorage.getItem("fluxos.welcome")) return;
    sessionStorage.removeItem("fluxos.welcome");
    const t = setTimeout(() => useShell.getState().show("assistant"), 900);
    return () => clearTimeout(t);
  }, [ready]);

  if (!spec) return <Connecting />;
  return (
    <div className="fixed inset-0 overflow-hidden">
      <Wallpaper />
      {!spec.desktop.windows.tiling && <DesktopIcons />}
      <WindowLayer spec={spec} />
      <SnapPreview />
      <Panel spec={spec} />
      {spec.desktop.dock.enabled && <Dock spec={spec} />}
      <Banners spec={spec} />
      <Toasts spec={spec} />
      <Assistant />
      <ControlCenter spec={spec} />
      <Launcher spec={spec} />
      <ConfirmDialog />
      <RestartOverlay />
    </div>
  );
}
