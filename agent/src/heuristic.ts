// Local rule-based fast path (PRD: "local rule-based fast path for common
// requests, with the LLM used for novel ones"). It costs nothing, works
// offline, and reports how much of the request it understood so the router
// can hand novel requests to an LLM.

import type { Context } from "./context.js";
import { tokenize } from "./retrieval.js";
import type { Draft, PackageEntry, Question, RawOp } from "./types.js";

export interface HeuristicResult {
  draft: Draft;
  coverage: number;
  rules: string[];
  confident: boolean;
}

const FILLER = new Set(
  tokenize(
    "make change switch turn want give please could would look feel like everything system computer machine pc os thing stuff bit little more much really very also now everyone today should something way kind sort lot new nice cool good better best great all",
  ),
);

const COLORS: Record<string, string> = {
  red: "#ef4444", crimson: "#dc143c", scarlet: "#ff2400", orange: "#f97316", amber: "#f59e0b", yellow: "#eab308",
  gold: "#d4a017", lime: "#84cc16", green: "#22c55e", emerald: "#10b981", mint: "#3eb489", teal: "#14b8a6",
  cyan: "#06b6d4", turquoise: "#40e0d0", sky: "#0ea5e9", blue: "#3b82f6", navy: "#1e3a8a", indigo: "#6366f1",
  violet: "#8b5cf6", purple: "#a855f7", lavender: "#9f7aea", magenta: "#d946ef", pink: "#ec4899", rose: "#f43f5e",
  brown: "#a0522d", coral: "#ff7f50", gray: "#6b7280", grey: "#6b7280", slate: "#64748b",
};
const NEON: Record<string, string> = {
  green: "#39ff14", pink: "#ff2e97", blue: "#00e5ff", cyan: "#00e5ff", purple: "#b026ff", orange: "#ff6b00", yellow: "#fff01f", red: "#ff073a",
};

const ALIASES: Record<string, string> = {
  "vs code": "vscode", "visual studio code": "vscode", "code editor": "vscodium", office: "libreoffice", chrome: "chromium",
  minecraft: "prismlauncher", obs: "obs-studio", "password manager": "keepassxc", email: "thunderbird", "video calls": "jitsi-meet",
  "packet capture": "wireshark", snmp: "net-snmp", netconf: "netconf-client", "remote desktop": "remmina", proton: "protonup-qt",
};

const CITY_TZ: Record<string, string> = {
  london: "Europe/London", berlin: "Europe/Berlin", paris: "Europe/Paris", madrid: "Europe/Madrid", amsterdam: "Europe/Amsterdam",
  "new york": "America/New_York", chicago: "America/Chicago", denver: "America/Denver", "los angeles": "America/Los_Angeles",
  "san francisco": "America/Los_Angeles", seattle: "America/Los_Angeles", toronto: "America/Toronto", "sao paulo": "America/Sao_Paulo",
  tokyo: "Asia/Tokyo", seoul: "Asia/Seoul", singapore: "Asia/Singapore", dubai: "Asia/Dubai", mumbai: "Asia/Kolkata",
  delhi: "Asia/Kolkata", bangalore: "Asia/Kolkata", bengaluru: "Asia/Kolkata", india: "Asia/Kolkata", sydney: "Australia/Sydney",
  utc: "UTC",
};

const PROFILES: { id: string; re: RegExp; label: string }[] = [
  { id: "gaming", re: /\b(gaming|gamer|games|esports?|play (pc )?games|steam deck)\b/, label: "gaming" },
  { id: "education", re: /\b(school|classroom|class room|students?|teachers?|education(al)?|universit(y|ies)|college|campus|computer lab|training (center|centre))\b/, label: "education" },
  { id: "telecom", re: /\b(telecom|telco|noc|network operations?|operations? (center|centre|desk)|isp|carrier|network engineers?)\b/, label: "telecom operations" },
  { id: "finance", re: /\b(trading|traders?|finance|financial|banking|bank|stock market|markets desk|hedge fund)\b/, label: "finance" },
  { id: "healthcare", re: /\b(hospital|clinic(al)?|healthcare|health care|nurs(e|es|ing)|doctors?|patients?|ehr|medical)\b/, label: "healthcare" },
];

const SETUP = /\b(set (this|it|me|us|everything)? ?up|setup|configure|turn (this|it) into|make (this|it) (a|an|into|for|ready)|prepare|for (a|an|my|our|the)?|optimi[sz]e|priorit[iy][sz]e|rig|workstation|desk|station|machine|pc|mode|lab|laptops?|computers?)\b/;

class Builder {
  ops: RawOp[] = [];
  covered = new Set<string>();
  rules: string[] = [];
  labels: string[] = [];
  questions: Question[] = [];
  kind: Draft["kind"] = "change";
  answer?: string;
  rollbackTo: number | null = null;

  constructor(readonly text: string) {}

  /** Matches and marks the matched words as understood. */
  hit(re: RegExp): RegExpMatchArray | null {
    const m = this.text.match(re);
    if (m) for (const t of tokenize(m[0])) this.covered.add(t);
    return m;
  }

  hitAll(re: RegExp): RegExpMatchArray[] {
    const flags = re.flags.includes("g") ? re.flags : re.flags + "g";
    const all = [...this.text.matchAll(new RegExp(re.source, flags))];
    for (const m of all) for (const t of tokenize(m[0])) this.covered.add(t);
    return all;
  }

  op(tool: string, params: Record<string, unknown>, reason: string) {
    const existing = this.ops.find((o) => o.tool === tool);
    if (existing && tool !== "packages.install" && tool !== "apps.install") {
      Object.assign(existing.params, params);
      return;
    }
    this.ops.push({ tool, params, reason });
  }

  rule(id: string, label: string) {
    this.rules.push(id);
    if (!this.labels.includes(label)) this.labels.push(label);
  }
}

function title(s: string): string {
  return s.replace(/\b\w/g, (c) => c.toUpperCase());
}

function mentioned(text: string, ctx: Context): PackageEntry[] {
  const found = new Map<string, PackageEntry>();
  for (const [alias, id] of Object.entries(ALIASES)) {
    if (new RegExp(`\\b${alias}\\b`).test(text)) {
      const p = ctx.catalog.packages.find((x) => x.id === id);
      if (p) found.set(p.id, p);
    }
  }
  for (const p of ctx.catalog.packages) {
    const names = [p.id, p.name.toLowerCase()].map((n) => n.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
    if (names.some((n) => new RegExp(`(^|[^a-z0-9-])${n}($|[^a-z0-9-])`).test(text))) found.set(p.id, p);
  }
  return [...found.values()];
}

export function heuristic(prompt: string, ctx: Context, answers: Record<string, string>, threshold: number): HeuristicResult | null {
  const text = prompt.toLowerCase().replace(/\s+/g, " ").trim();
  const b = new Builder(text);
  const spec = ctx.system.spec;
  const hasNvidia = ctx.hardware.gpus.some((g) => g.vendor === "nvidia");
  const changeVerb = /\b(install|add|make|set|change|enable|disable|switch|apply|turn|remove|uninstall|open|close|lock|use|give|prioriti[sz]e|optimi[sz]e|configure|block|allow|wipe|delete|run|undo|revert|reset|harden)\b/;

  // ---- questions about the system (answered locally when possible)
  if (/^(what|which|why|how|when|show|list|tell me|status|explain|is |are |do |does )/.test(text) && !changeVerb.test(text)) {
    const s = spec;
    let answer: string | undefined;
    if (b.hit(/\b(changed|history|recent|generations?|last change)\b/)) answer = "history";
    else if (b.hit(/\b(kernel|scheduler)\b/)) answer = `Kernel ${s.kernel.variant}, sched_ext scheduler ${s.kernel.sched_ext}, CPU governor ${s.kernel.cpu_governor}.`;
    else if (b.hit(/\b(theme|colou?rs?|fonts?|look)\b/))
      answer = `Theme "${s.design.theme_name}" (${s.design.tokens.scheme}), accent ${s.design.tokens.palette.accent}, ${s.design.tokens.typography.font_sans} for UI text, ${s.desktop.style} style on ${s.desktop.environment}.`;
    else if (b.hit(/\b(pending|approvals?|waiting)\b/)) answer = `${ctx.system.pending_approvals} plan(s) are waiting for approval.`;
    else if (b.hit(/\b(installed|apps|packages|software)\b/))
      answer = `System packages: ${s.packages.system.join(", ") || "none"}. User apps: ${s.packages.flatpak.join(", ") || "none"}. Services: ${s.services.enabled.join(", ")}.`;
    else if (b.hit(/\b(hardware|gpu|cpu|ram|memory)\b/)) answer = ctx.system.hardware;
    if (answer) {
      b.hitAll(/\b(what|which|why|how|when|show|list|tell me|status|explain|is|are|do|does|running|using|have|installed|current(ly)?|right now|am i|is my|my|been|lately|today)\b/);
      b.kind = "question";
      b.answer = answer;
      b.rule("question", "answer");
      return finish(b, threshold, prompt);
    }
    return null;
  }

  // ---- never-allowed requests: emit the blocked tool so fluxd refuses and audits it
  if (b.hit(/\b(wipe|erase|format|nuke|destroy)\b.*\b(disk|drive|ssd|hard ?drive|partitions?|everything)\b/)) {
    b.op("disk.wipe", {}, "user asked to wipe a disk");
    b.rule("blocked.disk", "disk wipe");
  }
  if (b.hit(/\b(disable|turn off|stop|kill|remove|drop)\b.*\bfirewall\b/)) {
    b.op("firewall.disable", {}, "user asked to disable the firewall");
    b.rule("blocked.firewall", "disable firewall");
  }
  if (b.hit(/\b(delete|clear|purge|remove|wipe|erase)\b.*\b(audit|audit log|logs?)\b/)) {
    b.op("audit.purge", {}, "user asked to delete audit logs");
    b.rule("blocked.audit", "delete audit logs");
  } else if (b.hit(/\b(disable|turn off|stop)\b.*\baudit(ing)?\b/)) {
    b.op("audit.disable", {}, "user asked to disable auditing");
    b.rule("blocked.audit", "disable auditing");
  }
  if (b.hit(/\b(run|execute|exec)\b.*\b(command|script|shell|bash|terminal)\b|\bsudo\b|rm -rf|\|\s*(ba)?sh\b/)) {
    b.op("shell.exec", { command: prompt.slice(0, 200) }, "user asked to run a command");
    b.rule("blocked.shell", "run a shell command");
  }

  // ---- undo / reset
  const undo = b.hit(/\b(undo|revert|roll ?back|go back|restore (the )?(previous|last|old|earlier))\b(.*?\bgeneration\s+(\d+))?/);
  if (undo) {
    b.hit(/\b(that|it|last change|changes?|version|generation)\b/);
    b.kind = "rollback";
    b.rollbackTo = undo[6] ? Number(undo[6]) : null;
    b.rule("undo", "undo");
    return finish(b, threshold, prompt);
  }
  if (b.hit(/\b(reset|restore|go back) (to )?(the )?(defaults?|factory( settings)?|original( settings)?|stock)\b|\bstart (over|fresh)\b/)) {
    b.op("profile.apply", { profile: "default", keep_existing: false }, "reset to the FluxOS default profile");
    b.rule("reset", "reset to defaults");
  }

  // ---- industry profiles
  const installVerb = /\b(install|add|get|download)\b/.test(text);
  for (const p of PROFILES) {
    const m = text.match(p.re);
    if (!m) continue;
    const contentCount = tokenize(text).filter((t) => !FILLER.has(t)).length;
    const setup = SETUP.test(text) || contentCount <= 4;
    if (!setup || (p.id === "gaming" && installVerb && !/\b(rig|setup|set up|prioriti|optimi|mode)\b/.test(text))) continue;
    b.hitAll(p.re);
    b.hitAll(SETUP);
    b.hitAll(/\b(prioriti[sz]e|above everything|over everything|everything else|first|ready|use|our|my)\b/);
    b.op("profile.apply", { profile: p.id }, `apply the ${p.label} profile`);
    b.rule(`profile.${p.id}`, `${p.label} profile`);
    if (p.id === "gaming" && hasNvidia && spec.drivers.gpu === "auto") {
      b.op("drivers.configure", { gpu: "nvidia-open" }, "NVIDIA GPU detected");
      b.rule("hw.nvidia", "NVIDIA open driver");
    }
    if (p.id === "education") {
      const said = /\b(lab|shared|public|library|personal|own|laptops?|home|1:1|one-to-one)\b/.exec(text);
      if (said) b.hit(/\b(lab|shared|public|library|personal|own|laptops?|home)\b/);
      const answer = answers.shared_lab ?? (said ? (/\b(lab|shared|public|library)\b/.test(said[0]) ? "shared" : "personal") : undefined);
      if (answer === undefined) {
        b.questions.push({
          id: "shared_lab",
          question: "Are these shared lab computers or personal devices?",
          why: "Shared lab machines can wipe the student account on every boot; personal devices must keep their files.",
          options: ["Shared lab: wipe the student account on every boot", "Personal devices: keep files"],
        });
      } else if (/^shared/i.test(answer)) {
        b.op("session.configure", { ephemeral_home: true }, "shared lab: clean state on every boot");
        b.rule("edu.lab", "clean state on every boot");
      }
    }
    if (p.id === "finance" || p.id === "healthcare") b.rule(`profile.${p.id}.preview`, "preview profile");
    break;
  }

  // ---- look-alike styles (original assets, neutral names)
  if (b.hit(/\b(mac ?os|macbook|apple|mac[- ]?like|like (a )?mac|cupertino)\b/)) {
    b.op("desktop.apply_style", { style: "glass-dock" }, "glass dock, global menu and smooth motion recreate that feel with original assets");
    b.op("design.set_typography", { font_sans: "Inter", font_display: "Inter" }, "clean neo-grotesque UI type");
    b.rule("style.glass-dock", "glass dock style");
  } else if (b.hit(/\bwindows ?(xp|7|vista)\b|\bclassic\b|\btraditional\b|\bold[- ]school\b/)) {
    b.op("desktop.apply_style", { style: "classic-panel" }, "single bottom panel");
    b.rule("style.classic", "classic panel");
  } else if (b.hit(/\b(like|feel like|look like|similar to) windows\b|\bwindows ?1[01]\b|\bwin ?1[01]\b|\bwindows[- ](like|style)\b/)) {
    b.op("desktop.apply_style", { style: "snap-layouts" }, "bottom taskbar with snap zones");
    b.rule("style.snap", "snap layouts style");
  }
  if (b.hit(/\bhyprland\b/)) {
    b.op("desktop.set_environment", { environment: "hyprland" }, "Hyprland tiling compositor");
    b.op("desktop.apply_style", { style: "tiling" }, "tiling layout");
    b.rule("env.hyprland", "Hyprland tiling");
  } else if (b.hit(/\b(tiling|tiled|tile (my |the )?windows|i3|sway|bspwm|xmonad|dwm)\b/)) {
    b.op("desktop.apply_style", { style: "tiling" }, "keyboard-driven tiling");
    b.rule("style.tiling", "tiling style");
  }
  if (b.hit(/\b(minimal(ist)?|distraction[- ]free|focus(ed)? (mode|layout)|zen mode|clean look)\b/)) {
    b.op("desktop.apply_style", { style: "focus" }, "calm, low-distraction layout");
    b.rule("style.focus", "focus style");
  }
  if (b.hit(/\b(ops[- ]cent(er|re)|command cent(er|re)|mission control|dashboard wall|information[- ]dense)\b/)) {
    b.op("desktop.apply_style", { style: "ops-center" }, "dense operations layout");
    b.rule("style.ops", "ops-center style");
  }
  const env = b.hit(/\b(use|switch to|change to|move to|install|i want|give me|try)\s+(kde|plasma|gnome|cosmic)\b/);
  if (env) {
    const e = env[2] === "kde" ? "plasma" : env[2]!;
    b.op("desktop.set_environment", { environment: e }, `switch to ${e}`);
    b.rule("env", `${e} desktop`);
  } else if (
    b.hit(/\b(use|switch to|change to|move to|go back to|back to|i want|give me)\s+(the\s+)?(fluxos|flux)\s+(shell|desktop)\b|\b(web|browser)[- ]based desktop\b/)
  ) {
    b.op("desktop.set_environment", { environment: "fluxos" }, "the FluxOS shell, the web desktop that follows the system spec");
    b.rule("env", "FluxOS shell");
  }

  // ---- colors and scheme
  const dark = b.hit(/\b(dark|night|black)\s*(mode|theme|look|ui)\b|\bgo dark\b|\bdarker\b|\bdark\b(?=.*\b(theme|mode)\b)/);
  const light = !dark && b.hit(/\b(light|bright|day|white)\s*(mode|theme|look|ui)\b|\blighter\b/);
  const scheme = dark ? "dark" : light ? "light" : undefined;
  const hexes = b.hitAll(/#[0-9a-f]{6}\b|#[0-9a-f]{3}\b/).map((m) => m[0]);
  const colorContext = /\b(colou?rs?|accent|theme|make (it|everything|this)|paint|tint|scheme|palette|brand|vibe|style|look|mode|neon)\b/.test(text);
  const named = Object.keys(COLORS).filter((c) => new RegExp(`\\b${c}(ish)?\\b`).test(text));
  if (hexes.length) {
    b.hit(/\b(brand|colou?rs?|accent|primary|secondary|palette)\b/);
    b.op("design.generate_palette", { accent: hexes[0], ...(hexes[1] ? { secondary: hexes[1] } : {}), ...(scheme ? { scheme } : {}) }, "palette derived from the given brand colors");
    b.rule("color.hex", "brand palette");
  } else if (named.length && colorContext) {
    const neon = b.hit(/\bneon\b/);
    const [first, second] = named;
    for (const c of named) b.hit(new RegExp(`\\b${c}(ish)?\\b`));
    b.hit(/\b(colou?rs?|accent|theme|palette|scheme|vibe)\b/);
    const pick = (c: string) => (neon && NEON[c] ? NEON[c] : COLORS[c]!);
    const accent = pick(first!);
    const name = `${neon ? "Neon " : ""}${title(first!)}${scheme === "light" ? " Day" : " Night"}`;
    b.op(
      "design.generate_palette",
      { accent, ...(second ? { secondary: pick(second) } : {}), scheme: scheme ?? (neon ? "dark" : spec.design.tokens.scheme), theme_name: name },
      `${named.join(" and ")} palette with contrast-checked text`,
    );
    b.rule("color.named", `${named.join("/")} palette`);
  } else if (scheme) {
    b.op("design.set_palette", { scheme }, `${scheme} mode`);
    b.rule("scheme", `${scheme} mode`);
  }

  // ---- typography and accessibility
  for (const f of ctx.catalog.fonts) {
    const fam = f.family.toLowerCase();
    if (text.includes(fam)) {
      b.hit(new RegExp(fam.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
      b.hit(/\b(fonts?|typeface|type|text)\b/);
      const field = f.kind === "mono" ? "font_mono" : f.kind === "display" ? "font_display" : "font_sans";
      b.op("design.set_typography", { [field]: f.family }, `${f.family} font`);
      b.rule("font", `${f.family} font`);
    }
  }
  if (b.hit(/\b(bigger|larger|increase|bump up|enlarge)\b.*\b(text|fonts?|type|letters)\b|\b(text|fonts?) (bigger|larger)\b|\blarge text\b/)) {
    b.op("accessibility.set", { large_text: true, text_scale: Math.min(2, Math.max(1.25, spec.accessibility.text_scale + 0.15)) }, "larger text everywhere");
    b.rule("a11y.large", "larger text");
  } else if (b.hit(/\b(smaller|decrease|reduce|shrink)\b.*\b(text|fonts?)\b/)) {
    b.op("design.set_typography", { base_size_px: Math.max(11, spec.design.tokens.typography.base_size_px - 1) }, "slightly smaller text");
    b.op("accessibility.set", { large_text: false, text_scale: 1 }, "normal text scale");
    b.rule("a11y.small", "smaller text");
  }
  if (b.hit(/\bdyslexi(a|c)\b/)) {
    b.op("design.set_typography", { font_sans: "Lexend", line_height: 1.7 }, "Lexend and wider line spacing aid reading fluency");
    b.rule("a11y.dyslexia", "dyslexia-friendly type");
  }
  if (b.hit(/\bhigh[- ]contrast\b|\bmore contrast\b|\blow vision\b|\bvisually impaired\b/)) {
    b.op("accessibility.set", { high_contrast: true }, "WCAG AAA contrast");
    b.rule("a11y.contrast", "high contrast");
  }
  if (b.hit(/\bscreen ?reader\b|\bblind\b|\bnarrator\b/)) {
    b.op("accessibility.set", { screen_reader: true }, "Orca screen reader");
    b.rule("a11y.reader", "screen reader");
  }
  if (b.hit(/\b(reduce|less|no|disable|turn off|stop|remove)\b.*\b(motion|animations?)\b|\bmotion sick(ness)?\b/)) {
    b.op("accessibility.set", { reduce_motion: true }, "reduced motion");
    b.op("design.set_motion", { preset: "none" }, "animations off");
    b.rule("a11y.motion", "reduced motion");
  } else if (b.hit(/\b(snappier|snappy|faster|quicker|speed up)\b.*\banimations?\b|\banimations?\b.*\b(faster|snappier|quicker)\b/)) {
    b.op("design.set_motion", { preset: "snappy" }, "snappier animations");
    b.rule("motion.snappy", "snappy motion");
  } else if (b.hit(/\b(smooth(er)?|fluid|buttery)\b/)) {
    b.op("design.set_motion", { preset: "smooth" }, "smooth motion");
    b.rule("motion.smooth", "smooth motion");
  } else if (b.hit(/\b(calm(er)?|gentle|slow(er)?)\b.*\banimations?\b/)) {
    b.op("design.set_motion", { preset: "calm" }, "calm motion");
    b.rule("motion.calm", "calm motion");
  }

  // ---- effects
  if (b.hit(/\b(no|less|remove|disable|without)\s+(blur|transparency|glass)\b|\bopaque\b|\bsolid (surfaces|panels|windows)\b/)) {
    b.op("design.set_effects", { blur_px: 0, transparency: 0 }, "opaque surfaces");
    b.op("desktop.set_windows", { blur: false }, "no window blur");
    b.rule("fx.off", "opaque surfaces");
  } else if (b.hit(/\b(glass(y)?|frosted|translucent|transparent|blurr?(y)?|acrylic|mica)\b/)) {
    b.op("design.set_effects", { blur_px: 32, transparency: 0.3 }, "frosted glass surfaces");
    b.op("desktop.set_windows", { blur: true }, "window blur");
    b.rule("fx.glass", "frosted glass");
  }

  // ---- wallpaper
  if (b.hit(/\b(wallpaper|background|backdrop)\b/)) {
    const style = b.hit(/\b(aurora|northern lights)\b/) ? "aurora"
      : b.hit(/\bgradient\b/) ? "gradient"
      : b.hit(/\b(mesh|blobs?|abstract)\b/) ? "mesh"
      : b.hit(/\b(grid|cyber(punk)?|synthwave|tron)\b/) ? "grid"
      : b.hit(/\b(solid|plain|flat)\b/) ? "solid"
      : spec.desktop.wallpaper.style;
    b.hit(/\b(new|different|another|change|fresh|random)\b/);
    b.op("desktop.set_wallpaper", { style, seed: Math.floor(Math.random() * 9000) + 100 }, `new ${style} wallpaper`);
    b.rule("wallpaper", `${style} wallpaper`);
  }

  // ---- dock, panel, workspaces, windows
  if (b.hit(/\bdock\b/)) {
    const layout: Record<string, unknown> = {};
    if (b.hit(/\b(no|remove|disable|get rid of|without)\b.*\bdock\b|\bdock\b.*\b(off|away)\b/)) layout.dock_enabled = false;
    else {
      layout.dock_enabled = true;
      const pos = b.hit(/\b(left|right|bottom)\b/);
      if (pos) layout.dock_position = pos[1];
      if (b.hit(/\bauto-?hide|hide (the )?dock|hidden\b/)) layout.dock_autohide = true;
      if (b.hit(/\b(bigger|larger)\b/)) layout.dock_size_px = Math.min(96, spec.desktop.dock.size_px + 12);
      if (b.hit(/\b(smaller|tiny)\b/)) layout.dock_size_px = Math.max(24, spec.desktop.dock.size_px - 12);
      if (b.hit(/\bfloating\b/)) layout.dock_floating = true;
    }
    b.op("desktop.set_layout", layout, "dock layout");
    b.rule("dock", "dock layout");
  }
  if (b.hit(/\b(panel|taskbar|top bar|menu bar|global menu)\b/)) {
    const layout: Record<string, unknown> = {};
    const pos = b.hit(/\b(top|bottom)\b/);
    if (pos) layout.panel_position = pos[1];
    if (b.hit(/\bglobal menu|menu bar\b/)) layout.global_menu = true;
    if (Object.keys(layout).length) {
      b.op("desktop.set_layout", layout, "panel layout");
      b.rule("panel", "panel layout");
    }
  }
  const ws = b.hit(/\b(\d{1,2})\s*(workspaces|virtual desktops|desktops)\b/);
  if (ws) {
    b.op("desktop.set_layout", { workspaces: Math.min(12, Math.max(1, Number(ws[1]))) }, "workspace count");
    b.rule("workspaces", `${ws[1]} workspaces`);
  }
  if (b.hit(/\b(rounded|round(er)?|soft(er)?) corners\b/)) {
    b.op("desktop.set_windows", { corner_radius_px: 14 }, "rounder windows");
    b.op("design.set_shape", { radius_sm_px: 8, radius_md_px: 12, radius_lg_px: 18 }, "rounder controls");
    b.rule("corners.round", "rounded corners");
  } else if (b.hit(/\b(square|sharp)( corners)?\b/)) {
    b.op("desktop.set_windows", { corner_radius_px: 0 }, "square windows");
    b.op("design.set_shape", { radius_sm_px: 2, radius_md_px: 2, radius_lg_px: 4 }, "square controls");
    b.rule("corners.square", "square corners");
  }
  if (b.hit(/\bsnap(ping)?\b/) && !b.rules.includes("style.snap")) {
    b.op("desktop.set_windows", { snapping: true }, "window snapping");
    b.rule("snap", "window snapping");
  }

  // ---- services
  const disableVerb = /\b(disable|turn off|stop|deactivate|remove|uninstall|kill)\b/;
  const services: [RegExp, string][] = [
    [/\b(ssh|sshd|remote (login|shell|access))\b/, "ssh"], [/\bbluetooth\b/, "bluetooth"], [/\bprint(er|ers|ing)\b/, "printing"],
    [/\b(tailscale|vpn)\b/, "tailscale"], [/\bdocker\b/, "docker"], [/\bpodman\b/, "podman"],
    [/\b(virtual machines?|vms?|virtuali[sz]ation|libvirt|kvm)\b/, "libvirtd"], [/\b(time sync|ntp|chrony|precise time|clock sync)\b/, "chrony"],
    [/\bgame ?mode\b/, "gamemode"], [/\b(mdns|avahi|bonjour)\b/, "avahi"], [/\b(smart ?cards?|badge( login| readers?)?)\b/, "smartcard"],
    [/\b(firmware updates?|fwupd)\b/, "fwupd"], [/\bflatpak|flathub\b/, "flatpak"],
  ];
  for (const [re, id] of services) {
    if (!b.hit(re)) continue;
    const off = disableVerb.test(text);
    b.hit(/\b(enable|turn on|start|activate|install|add|need|set up|disable|turn off|stop|deactivate|remove|uninstall|server|service)\b/);
    b.op(off ? "services.disable" : "services.enable", { services: [id] }, `${off ? "disable" : "enable"} ${id}`);
    b.rule(`svc.${id}`, `${off ? "disable" : "enable"} ${id}`);
  }

  // ---- apps (catalog lookups only)
  const removing = /\b(remove|uninstall|delete|get rid of)\b/.test(text);
  const installing = /\b(install|add|get( me)?|download|i (want|need)|set up|give me)\b/.test(text);
  if (installing || removing) {
    for (const p of mentioned(text, ctx)) {
      if (b.ops.some((o) => o.tool === "profile.apply")) break;
      b.hit(new RegExp(`\\b(${p.id}|${p.name.toLowerCase().replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})\\b`));
      for (const [alias, id] of Object.entries(ALIASES)) if (id === p.id) b.hit(new RegExp(`\\b${alias}\\b`));
      b.hit(/\b(install|add|get|download|want|need|remove|uninstall|delete|get rid of|apps?|application|program)\b/);
      if (removing) {
        if (spec.packages.flatpak.includes(p.id)) b.op("apps.remove", { apps: [p.id] }, `remove ${p.name}`);
        else if (spec.packages.system.includes(p.id)) b.op("packages.remove", { packages: [p.id] }, `remove ${p.name}`);
        else continue;
        b.rule(`app.remove.${p.id}`, `remove ${p.name}`);
        continue;
      }
      const system = p.nix.kind === "module" || !p.flatpak || /\b(system[- ]wide|system package|nix)\b/.test(text);
      const existing = b.ops.find((o) => o.tool === (system ? "packages.install" : "apps.install"));
      if (existing) (existing.params[system ? "packages" : "apps"] as string[]).push(p.id);
      else b.op(system ? "packages.install" : "apps.install", system ? { packages: [p.id] } : { apps: [p.id] }, system ? `${p.name} needs system integration` : `${p.name} as a sandboxed Flathub app`);
      b.rule(`app.install.${p.id}`, `install ${p.name}`);
    }
  }

  // ---- firewall ports
  const ports = b.hit(/\b(open|allow|expose|unblock|close|block|deny)\b.*?\bports?\s+((\d{1,5})(\s*\/\s*(tcp|udp))?(\s*(,|and)\s*\d{1,5}(\s*\/\s*(tcp|udp))?)*)/);
  if (ports) {
    const close = /^(close|block|deny)$/.test(ports[1]!);
    const tcp: number[] = [];
    const udp: number[] = [];
    for (const m of ports[2]!.matchAll(/(\d{1,5})(\s*\/\s*(tcp|udp))?/g)) {
      const n = Number(m[1]);
      if (n >= 1 && n <= 65535) (m[3] === "udp" ? udp : tcp).push(n);
    }
    b.op(close ? "firewall.revoke" : "firewall.allow", { ...(tcp.length ? { tcp } : {}), ...(udp.length ? { udp } : {}) }, `${close ? "close" : "open"} inbound ports`);
    b.rule("firewall", `${close ? "close" : "open"} ports`);
  }

  // ---- kernel and performance
  const kernels: [RegExp, string][] = [
    [/\b(zen|low[- ]latency) kernel\b/, "low-latency"], [/\b(lts|long[- ]term|stable) kernel\b/, "lts"],
    [/\b(latest|newest|bleeding[- ]edge) kernel\b/, "latest"], [/\bxanmod\b|\bperformance kernel\b/, "performance"],
  ];
  for (const [re, variant] of kernels) {
    if (b.hit(re)) {
      b.hit(/\b(switch|use|install|kernel)\b/);
      b.op("kernel.select", { variant }, `${variant} kernel`);
      b.rule(`kernel.${variant}`, `${variant} kernel`);
    }
  }
  if (b.hit(/\bharden(ed|ing)?\b|\bmore secure\b|\block (it|this|the system) down\b|\bsecure (it|this|the system)\b/)) {
    b.hit(/\b(kernel|system|security|machine)\b/);
    b.op("kernel.select", { variant: "hardened" }, "LTS kernel with hardening parameters");
    b.op("security.configure", { mac: "apparmor", screen_lock_enabled: true }, "AppArmor and auto-lock");
    b.rule("harden", "hardened kernel and policy");
  }
  const sched = b.hit(/\b(scx_)?(lavd|bpfland|rusty|flash|cosmos|p2dq)\b/);
  if (sched) {
    b.hit(/\b(scheduler|sched_ext|scx)\b/);
    b.op("kernel.set_scheduler", { scheduler: sched[2] }, `sched_ext ${sched[2]}`);
    b.rule("sched", `${sched[2]} scheduler`);
  } else if (b.hit(/\b(disable|turn off|remove|default)\b.*\b(sched_ext|scx|custom scheduler)\b/)) {
    b.op("kernel.set_scheduler", { scheduler: "off" }, "kernel default scheduler");
    b.rule("sched.off", "default scheduler");
  }
  if (!b.rules.some((r) => r.startsWith("profile."))) {
    if (b.hit(/\b(low(er)? latency|reduce latency|less lag|input lag|more responsive|responsiveness|stutter(ing)?|more fps|higher fps)\b/)) {
      b.op("kernel.set_scheduler", { scheduler: "lavd" }, "latency-aware scheduler, no reboot needed");
      b.op("kernel.tune", { cpu_governor: "performance" }, "keep CPU clocks up");
      b.rule("perf.latency", "lower latency");
    } else if (b.hit(/\b(max(imum)? performance|performance mode|speed (it|this|things)? ?up|boost( performance)?|run faster|make it faster|faster)\b/) && !b.rules.includes("motion.snappy")) {
      b.op("kernel.tune", { cpu_governor: "performance" }, "performance governor");
      b.op("kernel.set_scheduler", { scheduler: "bpfland" }, "interactive scheduler");
      b.rule("perf.boost", "performance mode");
    } else if (b.hit(/\b(battery( life)?|power sav(ing|er)|save power|energy saving|quieter|run cooler)\b/)) {
      b.op("kernel.tune", { cpu_governor: "powersave" }, "power-saving governor");
      b.op("kernel.set_scheduler", { scheduler: "off" }, "default scheduler");
      b.rule("perf.battery", "power saving");
    }
  }

  // ---- security and session
  const lock = b.hit(/\b(auto-?lock|lock (the )?(screen|computer|pc|machine)?)\b(.*?\b(after|in|every)\s+(\d{1,3})\s*(min(ute)?s?|m)\b)?/);
  if (lock) {
    const minutes = lock[6] ? Math.min(240, Math.max(1, Number(lock[6]))) : 5;
    b.op("security.configure", { screen_lock_enabled: true, screen_lock_idle_minutes: minutes }, `lock after ${minutes} min idle`);
    b.rule("lock", `auto-lock ${minutes} min`);
  }
  if (b.hit(/\b(block|disable|disallow|forbid)\b.*\busb\b/)) {
    b.op("security.configure", { usb_storage: "block" }, "block USB storage");
    b.rule("usb.block", "block USB storage");
  } else if (b.hit(/\b(allow|enable|unblock)\b.*\busb\b/)) {
    b.op("security.configure", { usb_storage: "allow" }, "allow USB storage");
    b.rule("usb.allow", "allow USB storage");
  }
  const url = prompt.match(/https:\/\/[^\s"'<>]+/)?.[0] ?? answers.kiosk_url;
  const kiosk = b.hit(/\bkiosk\b/);
  const exam = b.hit(/\bexam(s|ination)?( mode| lock-?down)?\b/);
  if (b.hit(/\b(exit|leave|disable|turn off|stop) (kiosk|exam)( mode)?\b|\bnormal desktop\b/)) {
    b.op("session.configure", { mode: "standard" }, "back to the normal desktop");
    b.rule("session.standard", "normal desktop");
  } else if (kiosk || exam) {
    b.hit(/\b(mode|lock-?down|lock)\b/);
    if (url) {
      for (const t of tokenize(url)) b.covered.add(t);
      b.op("session.configure", { mode: exam ? "exam" : "kiosk", kiosk_url: url }, `${exam ? "exam" : "kiosk"} session`);
      if (exam) b.op("security.configure", { usb_storage: "block" }, "exams block USB storage");
      b.rule(`session.${exam ? "exam" : "kiosk"}`, `${exam ? "exam" : "kiosk"} mode`);
    } else {
      b.rule(`session.${exam ? "exam" : "kiosk"}`, `${exam ? "exam" : "kiosk"} mode`);
      b.questions.push({ id: "kiosk_url", question: `Which https address should the ${exam ? "exam" : "kiosk"} screen open?`, why: "The locked-down session shows a single web app." });
    }
  }
  if (b.hit(/\b(wipe|reset|clear|erase) (the )?(user |student |guest )?(data|files|home|account)\b|\bclean state\b|\bguest (account|mode)\b|\bshared (computer|machine|pc|lab|device)s?\b/)) {
    b.hit(/\b(every|each|on|after|logout|log out|reboot|restart|boot|login)\b/);
    b.op("session.configure", { ephemeral_home: true }, "shared account wiped on every boot");
    b.rule("session.ephemeral", "clean state on every boot");
  }

  // ---- boot splash
  const splash = ctx.catalog.splash_themes.find((t) => text.includes(t.replace(/_/g, " ")) || text.includes(t));
  if (b.hit(/\b(boot ?(splash|animation|screen|logo|up)|splash screen|startup animation|plymouth)\b/) || splash) {
    if (b.hit(/\b(no|disable|remove|verbose|text)\b.*\b(splash|animation)\b/)) b.op("boot.configure", { splash: false }, "verbose text boot");
    else {
      const theme = splash ?? (b.hit(/\b(cool|futuristic|sci-?fi|hud|techy)\b/) ? "circle_hud" : "hexagon_dots");
      if (splash) b.hit(new RegExp(splash.replace(/_/g, "[ _]")));
      b.op("boot.configure", { splash: true, splash_theme: theme }, `${theme} boot animation`);
    }
    b.rule("boot", "boot splash");
  }

  // ---- identity
  const host = prompt.match(/\b(?:hostname|computer name|machine name)\b.*?\bto\s+([a-z0-9][a-z0-9-]{0,62})\b/i) ?? prompt.match(/\b(?:rename|call)\s+(?:this|it|the computer|the machine|this machine)\s+["']?([a-z0-9][a-z0-9-]{0,62})/i);
  if (host) {
    b.hit(/\b(hostname|computer name|machine name|rename|call)\b/);
    b.covered.add(host[1]!.toLowerCase());
    b.op("system.set_identity", { hostname: host[1]!.toLowerCase() }, "new hostname");
    b.rule("hostname", "hostname");
  }
  const tzExplicit = prompt.match(/\b([A-Z][a-z]+\/[A-Z][A-Za-z_]+)\b/)?.[1];
  const city = Object.keys(CITY_TZ).find((c) => new RegExp(`\\b${c}\\b`).test(text));
  if (/\b(time ?zone|timezone|clock)\b/.test(text) && (tzExplicit || city)) {
    b.hit(/\b(time ?zone|timezone|clock)\b/);
    if (city) b.hit(new RegExp(`\\b${city}\\b`));
    if (tzExplicit) for (const t of tokenize(tzExplicit)) b.covered.add(t);
    b.op("system.set_identity", { timezone: tzExplicit ?? CITY_TZ[city!] }, "time zone");
    b.rule("timezone", "time zone");
  }

  if (!b.rules.length) return null;
  if (b.questions.length) b.kind = "clarify";
  return finish(b, threshold, prompt);
}

function finish(b: Builder, threshold: number, prompt: string): HeuristicResult {
  const content = tokenize(prompt).filter((t) => !FILLER.has(t));
  const covered = content.filter((t) => b.covered.has(t)).length;
  const coverage = content.length ? covered / content.length : 1;
  const summary =
    b.kind === "rollback" ? "Undo the last change" : b.kind === "question" ? "Answer from the current system state" : b.labels.join(", ");
  return {
    draft: {
      kind: b.kind,
      summary: summary.charAt(0).toUpperCase() + summary.slice(1),
      rationale: b.kind === "change" ? "Mapped by the local rule engine (no AI provider call)." : undefined,
      answer: b.answer,
      questions: b.questions.length ? b.questions : undefined,
      operations: b.kind === "change" || b.kind === "clarify" ? b.ops : [],
      rollback_to: b.rollbackTo,
    },
    coverage: Math.round(coverage * 100) / 100,
    rules: b.rules,
    confident: coverage >= threshold,
  };
}
