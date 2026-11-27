import type { LucideIcon } from "lucide-react";
import { ArrowRight, LoaderCircle, Power } from "lucide-react";
import { Fragment, type ButtonHTMLAttributes, type ReactNode } from "react";

import { TIER_LABEL, isHexColor, tierTone } from "../lib/format";
import type { Change, Tier } from "../lib/types";

type Variant = "primary" | "secondary" | "ghost" | "danger" | "subtle";

const VARIANT: Record<Variant, string> = {
  primary: "bg-accent text-on-accent hover:brightness-110 active:brightness-95 shadow-[0_8px_24px_-12px_var(--flux-color-accent)]",
  secondary: "border hairline bg-raised text-fg hover:border-accent/50",
  ghost: "text-fg/85 hover:bg-fg/[0.07] hover:text-fg",
  danger: "bg-bad/15 text-bad hover:bg-bad/25",
  subtle: "bg-fg/[0.06] text-fg hover:bg-fg/[0.1]",
};

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: "sm" | "md";
  icon?: LucideIcon;
  busy?: boolean;
}

export function Button({ variant = "secondary", size = "md", icon: Icon, busy, children, className = "", disabled, ...rest }: ButtonProps) {
  return (
    <button
      type="button"
      {...rest}
      disabled={disabled || busy}
      className={`inline-flex shrink-0 items-center justify-center gap-1.5 rounded-md font-medium whitespace-nowrap transition-[background-color,filter,border-color,opacity] disabled:pointer-events-none disabled:opacity-45 ${
        size === "sm" ? "h-7 px-2.5 text-xs" : "h-9 px-3.5 text-sm"
      } ${VARIANT[variant]} ${className}`}
    >
      {busy ? <LoaderCircle className="size-4 animate-spin" /> : Icon ? <Icon className="size-4" /> : null}
      {children}
    </button>
  );
}

export function IconButton({
  icon: Icon,
  label,
  className = "",
  size = 16,
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & { icon: LucideIcon; label: string; size?: number }) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      {...rest}
      className={`inline-flex items-center justify-center rounded-md text-fg/80 transition-colors hover:bg-fg/[0.08] hover:text-fg disabled:opacity-40 ${className}`}
    >
      <Icon style={{ width: size, height: size }} />
    </button>
  );
}

export function Spinner({ className = "size-4" }: { className?: string }) {
  return <LoaderCircle className={`${className} animate-spin`} aria-label="Working" />;
}

export function Kbd({ children }: { children: ReactNode }) {
  return (
    <kbd className="rounded border hairline bg-fg/[0.06] px-1.5 py-px font-mono text-[11px] text-muted shadow-[inset_0_-1px_0_rgb(0_0_0/0.2)]">
      {children}
    </kbd>
  );
}

export function TierBadge({ tier, label = true }: { tier: Tier; label?: boolean }) {
  const t = tierTone(tier);
  return (
    <span className={`inline-flex shrink-0 items-center gap-1.5 rounded-full px-2 py-0.5 text-xs font-semibold ${t.bg} ${t.text}`}>
      <span className={`size-1.5 rounded-full ${t.dot}`} />
      Tier {tier}
      {label && <span className="font-medium opacity-80">· {TIER_LABEL[tier]}</span>}
    </span>
  );
}

export function Chip({ children, className = "", title }: { children: ReactNode; className?: string; title?: string }) {
  return (
    <span title={title} className={`inline-flex items-center gap-1 rounded-full border hairline px-2 py-0.5 text-xs text-fg/85 ${className}`}>
      {children}
    </span>
  );
}

export function Swatch({ hex, size = 14, title }: { hex: string; size?: number; title?: string }) {
  return (
    <span
      title={title ?? hex}
      className="inline-block shrink-0 rounded-[4px] border border-fg/20 align-middle"
      style={{ background: hex, width: size, height: size }}
    />
  );
}

export function Section({ title, children, right, className = "" }: { title: ReactNode; children: ReactNode; right?: ReactNode; className?: string }) {
  return (
    <section className={className}>
      <div className="mb-2 flex items-center gap-2">
        <h3 className="text-xs font-semibold tracking-[0.12em] text-muted uppercase">{title}</h3>
        <div className="flex-1" />
        {right}
      </div>
      {children}
    </section>
  );
}

export function KV({ rows }: { rows: [ReactNode, ReactNode][] }) {
  return (
    <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-sm">
      {rows.map(([k, v], i) => (
        <div key={i} className="contents">
          <dt className="text-muted">{k}</dt>
          <dd className="min-w-0 truncate text-fg">{v}</dd>
        </div>
      ))}
    </dl>
  );
}

export function Empty({ icon: Icon, title, children }: { icon: LucideIcon; title: string; children?: ReactNode }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-2 p-8 text-center">
      <div className="grid size-12 place-items-center rounded-2xl bg-fg/[0.06] text-muted">
        <Icon className="size-6" />
      </div>
      <p className="font-semibold">{title}</p>
      {children && <div className="max-w-sm text-sm text-muted">{children}</div>}
    </div>
  );
}

export function Tabs<T extends string>({ value, options, onChange }: { value: T; options: { id: T; label: string }[]; onChange: (v: T) => void }) {
  return (
    <div role="tablist" className="inline-flex rounded-lg bg-fg/[0.06] p-0.5">
      {options.map((o) => (
        <button
          key={o.id}
          type="button"
          role="tab"
          aria-selected={value === o.id}
          onClick={() => onChange(o.id)}
          className={`rounded-md px-3 py-1 text-sm transition-colors ${value === o.id ? "bg-raised font-semibold text-fg shadow-sm" : "text-muted hover:text-fg"}`}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

/**
 * Text from fluxd or the agent. Change summaries read "before → after", but
 * the self-hosted Latin font subsets have no U+2192, so the arrow is drawn
 * as an icon instead of falling back to whatever system font exists.
 */
export function ArrowText({ text }: { text: string | null | undefined }) {
  if (!text) return null;
  const parts = text.split("→");
  if (parts.length === 1) return <>{text}</>;
  return (
    <>
      {parts.map((part, i) => (
        <Fragment key={i}>
          {i > 0 && (
            <>
              <ArrowRight aria-hidden className="inline-block size-[1em] align-[-0.15em] opacity-75" />
              <span className="sr-only">to</span>
            </>
          )}
          {part}
        </Fragment>
      ))}
    </>
  );
}

/** Validated changes from fluxd's diff, riskiest first. */
export function ChangeList({ changes, limit }: { changes: Change[]; limit?: number }) {
  const shown = limit ? changes.slice(0, limit) : changes;
  return (
    <ul className="divide-y divide-line/40">
      {shown.map((c, i) => (
        <li key={`${c.path}-${i}`} className="flex items-center gap-2.5 py-1.5 text-sm">
          <span className={`size-2 shrink-0 rounded-full ${tierTone(c.tier).dot}`} title={`Tier ${c.tier} · ${TIER_LABEL[c.tier]}`} />
          <span className="min-w-0 flex-1 truncate" title={c.summary}>
            <ArrowText text={c.summary} />
          </span>
          {isHexColor(c.before) && isHexColor(c.after) && (
            <span className="flex shrink-0 items-center gap-1">
              <Swatch hex={c.before} />
              <ArrowRight className="size-3 text-muted" />
              <Swatch hex={c.after} />
            </span>
          )}
          {c.requires_reboot && (
            <span title="Takes effect after a restart" className="text-warn">
              <Power className="size-3.5" />
            </span>
          )}
        </li>
      ))}
      {limit !== undefined && changes.length > limit && <li className="py-1.5 text-xs text-muted">+{changes.length - limit} more changes</li>}
    </ul>
  );
}

export function StatusDot({ tone, pulse }: { tone: "ok" | "warn" | "bad" | "accent" | "muted"; pulse?: boolean }) {
  const cls = { ok: "bg-ok", warn: "bg-warn", bad: "bg-bad", accent: "bg-accent", muted: "bg-muted" }[tone];
  return (
    <span className="relative inline-flex size-2">
      {pulse && <span className={`absolute inset-0 rounded-full ${cls} opacity-70 [animation:flux-pulse_1.6s_ease-in-out_infinite]`} />}
      <span className={`relative size-2 rounded-full ${cls}`} />
    </span>
  );
}
