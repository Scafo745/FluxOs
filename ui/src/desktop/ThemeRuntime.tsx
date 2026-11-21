import { useLayoutEffect } from "react";

import { selectSpec, useSystem } from "../state/system";

let morphTimer: ReturnType<typeof setTimeout> | null = null;

/**
 * Applies the OS theme to the page: fluxd's theme.css (or a preview's) plus
 * data attributes for blur, motion and scheme. Theme changes cross-fade.
 */
export function ThemeRuntime() {
  const css = useSystem((s) => s.preview?.css ?? s.themeCss);
  const spec = useSystem(selectSpec);

  useLayoutEffect(() => {
    if (!css) return;
    let el = document.getElementById("flux-theme") as HTMLStyleElement | null;
    const first = !el;
    if (!el) {
      el = document.createElement("style");
      el.id = "flux-theme";
      document.head.appendChild(el);
    }
    if (el.textContent === css) return;
    const root = document.documentElement;
    if (!first && root.dataset.motion !== "reduced") {
      root.classList.add("theme-morph");
      if (morphTimer) clearTimeout(morphTimer);
      morphTimer = setTimeout(() => root.classList.remove("theme-morph"), 900);
    }
    el.textContent = css;
  }, [css]);

  useLayoutEffect(() => {
    if (!spec) return;
    const root = document.documentElement;
    const t = spec.design.tokens;
    root.dataset.blur = spec.desktop.windows.blur && t.effects.blur_px > 0 ? "on" : "off";
    root.dataset.motion = spec.accessibility.reduce_motion || t.motion.animation_scale === 0 ? "reduced" : "full";
    root.dataset.scheme = t.scheme;
    root.dataset.contrast = spec.accessibility.high_contrast ? "high" : "normal";
  }, [spec]);

  return null;
}
