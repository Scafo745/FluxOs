import { Check, Sparkles, WandSparkles } from "lucide-react";
import { useState } from "react";

import { Button, Section, Swatch } from "../components/ui";
import type { RawOp } from "../lib/types";
import { quick } from "../state/actions";
import { useAssistant } from "../state/assistant";
import { useShell } from "../state/shell";
import { selectSpec, useSystem } from "../state/system";

/**
 * Looks from the MorphOS prototype, rebuilt as FluxOS plans. Each one is a
 * tier 0-1 change (palette, layout preset, wallpaper, motion) that fluxd
 * validates and applies at once with a snapshot, so it can be undone.
 */
const PRESETS: { name: string; blurb: string; accent: string; secondary: string; scheme: "dark" | "light"; style: string; wallpaper: string; motion: string }[] = [
  { name: "Vercel Geist", blurb: "Stark black on near-white, developer minimal", accent: "#0070f3", secondary: "#7928ca", scheme: "light", style: "focus", wallpaper: "solid", motion: "snappy" },
  { name: "Cupertino Glass", blurb: "Translucent glass dock and smooth motion", accent: "#0a84ff", secondary: "#bf5af2", scheme: "dark", style: "glass-dock", wallpaper: "aurora", motion: "smooth" },
  { name: "Gothic Obsidian", blurb: "Obsidian glass with blood-crimson light", accent: "#dc143c", secondary: "#8b0a2f", scheme: "dark", style: "glass-dock", wallpaper: "mesh", motion: "calm" },
  { name: "Cyber Matrix", blurb: "Phosphor green, keyboard-driven tiling", accent: "#00ff9c", secondary: "#00b3ff", scheme: "dark", style: "tiling", wallpaper: "grid", motion: "snappy" },
  { name: "Synthwave 1984", blurb: "Neon pink and cyan over a retro grid", accent: "#ff2e97", secondary: "#00e5ff", scheme: "dark", style: "glass-dock", wallpaper: "grid", motion: "smooth" },
  { name: "Royal Sovereign", blurb: "Gold leaf on deep violet", accent: "#d4af37", secondary: "#7c3aed", scheme: "dark", style: "glass-dock", wallpaper: "mesh", motion: "calm" },
  { name: "Kawaii Sakura", blurb: "Soft cherry blossom pastels", accent: "#ff6f9c", secondary: "#a78bfa", scheme: "light", style: "focus", wallpaper: "aurora", motion: "calm" },
  { name: "Nordic Arctic", blurb: "Cold blues and frost, calm and clear", accent: "#5e81ac", secondary: "#88c0d0", scheme: "dark", style: "focus", wallpaper: "gradient", motion: "calm" },
  { name: "FinTech Shield", blurb: "Enterprise blue with a snap-layout taskbar", accent: "#2563eb", secondary: "#0ea5e9", scheme: "light", style: "snap-layouts", wallpaper: "gradient", motion: "snappy" },
];

const IDEAS = ["gothic theme with crimson accents", "make it feel like macOS", "neon green and pink theme", "bigger text and calmer motion"];

function ops(p: (typeof PRESETS)[number]): RawOp[] {
  return [
    { tool: "desktop.apply_style", params: { style: p.style } },
    { tool: "design.generate_palette", params: { accent: p.accent, secondary: p.secondary, scheme: p.scheme, theme_name: p.name } },
    { tool: "desktop.set_wallpaper", params: { style: p.wallpaper, seed: Math.floor(Math.random() * 9000) + 100 } },
    { tool: "design.set_motion", params: { preset: p.motion } },
  ];
}

export function Synthesizer() {
  const spec = useSystem(selectSpec);
  const [prompt, setPrompt] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const run = (text: string) => {
    if (!text.trim()) return;
    useShell.getState().show("assistant");
    void useAssistant.getState().ask(text.trim());
    setPrompt("");
  };
  if (!spec) return null;
  const pal = spec.design.tokens.palette;

  return (
    <div className="h-full space-y-6 overflow-y-auto p-5">
      <div className="rounded-xl border hairline bg-accent/[0.06] p-4">
        <p className="flex items-center gap-2 font-display text-lg font-bold">
          <WandSparkles className="size-5 text-accent" /> Synthesize your OS from a prompt
        </p>
        <p className="mt-1 text-sm text-muted">
          The AI turns your words into a typed plan. fluxd checks it, applies looks at once with a snapshot, and asks you before it changes the system.
        </p>
        <form
          className="mt-3 flex gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            run(prompt);
          }}
        >
          <input
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            placeholder="e.g. a calm writing desk with warm colors"
            className="h-9 min-w-0 flex-1 rounded-md border hairline bg-bg/50 px-3 text-sm outline-none focus:border-accent"
          />
          <Button type="submit" variant="primary" icon={Sparkles} disabled={!prompt.trim()}>
            Synthesize
          </Button>
        </form>
        <div className="mt-2 flex flex-wrap gap-1.5">
          {IDEAS.map((i) => (
            <button key={i} type="button" onClick={() => run(i)} className="rounded-full border hairline px-2.5 py-0.5 text-xs text-muted hover:border-accent/60 hover:text-fg">
              {i}
            </button>
          ))}
        </div>
      </div>

      <Section title="Looks" right={<span className="text-xs text-muted">Live theme: {spec.design.theme_name}</span>}>
        <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
          {PRESETS.map((p) => {
            const current = spec.design.theme_name === p.name;
            return (
              <button
                key={p.name}
                type="button"
                disabled={busy !== null}
                onClick={async () => {
                  setBusy(p.name);
                  await quick(`Synthesizer: ${p.name}`, ops(p));
                  setBusy(null);
                }}
                className={`group relative overflow-hidden rounded-xl border p-3 text-left transition-colors ${current ? "border-accent bg-accent/10" : "hairline hover:border-accent/50"}`}
              >
                <span
                  className="mb-3 block h-16 rounded-lg"
                  style={{
                    background: `radial-gradient(circle at 20% 20%, ${p.accent}cc, transparent 55%), radial-gradient(circle at 80% 70%, ${p.secondary}aa, transparent 55%), ${p.scheme === "dark" ? "#0b0b12" : "#f6f6f8"}`,
                  }}
                />
                <span className="flex items-center gap-2 font-semibold">
                  {p.name}
                  {current && <Check className="size-4 text-accent" />}
                </span>
                <span className="mt-0.5 block text-xs text-muted">{p.blurb}</span>
                <span className="mt-2 flex items-center gap-1.5 text-[11px] text-muted">
                  <Swatch hex={p.accent} size={12} />
                  <Swatch hex={p.secondary} size={12} />
                  {p.scheme} · {p.style}
                </span>
                {busy === p.name && <span className="absolute inset-0 grid place-items-center bg-bg/40 text-sm">Applying…</span>}
              </button>
            );
          })}
        </div>
      </Section>

      <Section title="Current palette">
        <div className="flex flex-wrap gap-3 text-xs text-muted">
          {(["accent", "secondary", "background", "surface", "text", "success", "warning", "danger"] as const).map((k) => (
            <span key={k} className="flex items-center gap-1.5">
              <Swatch hex={pal[k]} size={16} /> {k}
            </span>
          ))}
        </div>
      </Section>
    </div>
  );
}
