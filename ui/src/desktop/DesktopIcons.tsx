import { Sparkles } from "lucide-react";
import { useState } from "react";

import { APPS, DESKTOP_APPS, TINT_BG } from "../apps/meta";
import { useShell } from "../state/shell";

/** Desktop shortcuts, as in the MorphOS prototype: double-click to open. */
export function DesktopIcons() {
  const area = useShell((s) => s.area);
  const [selected, setSelected] = useState<string | null>(null);
  const items = [
    { id: "assistant", label: "Ask Flux", icon: Sparkles, background: "linear-gradient(140deg, var(--flux-color-accent), var(--flux-color-secondary))", open: () => useShell.getState().show("assistant") },
    ...DESKTOP_APPS.map((id) => ({ id, label: APPS[id].title, icon: APPS[id].icon, background: TINT_BG[APPS[id].tint], open: () => useShell.getState().open(id) })),
  ];
  return (
    <nav
      aria-label="Desktop shortcuts"
      className="absolute z-[1] flex flex-col gap-1"
      style={{ left: area.x + 14, top: area.y + 14 }}
      onMouseDown={(e) => e.target === e.currentTarget && setSelected(null)}
    >
      {items.map(({ id, label, icon: Icon, background, open }) => (
        <button
          key={id}
          type="button"
          onClick={() => setSelected(id)}
          onDoubleClick={() => {
            setSelected(null);
            open();
          }}
          onKeyDown={(e) => e.key === "Enter" && open()}
          className={`flex w-20 flex-col items-center gap-1.5 rounded-lg p-2 text-center outline-none transition-colors focus-visible:ring-2 focus-visible:ring-accent ${selected === id ? "bg-accent/20" : "hover:bg-fg/[0.06]"}`}
        >
          <span className="grid size-12 place-items-center rounded-[28%] text-white shadow-[0_8px_20px_-10px_rgb(0_0_0/0.7),inset_0_1px_0_rgb(255_255_255/0.22)]" style={{ background }}>
            <Icon className="size-6" />
          </span>
          <span className="line-clamp-2 text-xs font-medium text-fg [text-shadow:0_1px_3px_rgb(0_0_0/0.55)]">{label}</span>
        </button>
      ))}
    </nav>
  );
}
