import { useId } from "react";

/**
 * The FluxOS mark: an orbit ring around a flowing stroke, drawn in the live
 * accent and secondary colors. `progress` fills the ring (boot progress).
 */
export function FluxMark({
  size = 32,
  progress,
  spinning = false,
  glow = false,
  draw = false,
  className = "",
}: {
  size?: number;
  progress?: number;
  spinning?: boolean;
  glow?: boolean;
  draw?: boolean;
  className?: string;
}) {
  const id = useId().replace(/[^a-zA-Z0-9_-]/g, "");
  const p = progress === undefined ? 0.74 : Math.max(0.03, Math.min(1, progress));
  return (
    <svg viewBox="0 0 100 100" width={size} height={size} className={className} aria-hidden="true">
      <defs>
        <linearGradient id={`${id}g`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" style={{ stopColor: "var(--flux-color-accent)" }} />
          <stop offset="1" style={{ stopColor: "var(--flux-color-secondary)" }} />
        </linearGradient>
        {glow && (
          <filter id={`${id}f`} x="-40%" y="-40%" width="180%" height="180%">
            <feGaussianBlur stdDeviation="2.6" result="b" />
            <feMerge>
              <feMergeNode in="b" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        )}
      </defs>
      {progress !== undefined && <circle cx="50" cy="50" r="40" fill="none" stroke="var(--flux-color-text)" strokeOpacity="0.1" strokeWidth="3.5" />}
      <g style={{ transformOrigin: "50px 50px", animation: spinning ? "flux-spin 2.6s linear infinite" : undefined }}>
        <circle
          cx="50"
          cy="50"
          r="40"
          fill="none"
          stroke={`url(#${id}g)`}
          strokeWidth="4.5"
          strokeLinecap="round"
          pathLength={100}
          strokeDasharray={`${p * 100} 100`}
          transform="rotate(-90 50 50)"
          filter={glow ? `url(#${id}f)` : undefined}
          style={{ transition: "stroke-dasharray var(--flux-duration-slow) var(--flux-easing)" }}
        />
      </g>
      <path
        d="M30 62 C 40 37, 58 68, 70 40"
        fill="none"
        stroke={`url(#${id}g)`}
        strokeWidth="8.5"
        strokeLinecap="round"
        pathLength={100}
        strokeDasharray={draw ? "100" : undefined}
        filter={glow ? `url(#${id}f)` : undefined}
        style={draw ? { animation: "flux-draw 1.2s var(--flux-easing) both" } : undefined}
      />
      <circle
        cx="70"
        cy="40"
        r="6"
        fill="var(--flux-color-text)"
        style={{ transformOrigin: "70px 40px", animation: spinning ? "flux-pulse 2.4s ease-in-out infinite" : undefined }}
      />
    </svg>
  );
}
