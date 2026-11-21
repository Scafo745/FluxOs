# Design and look-alike styles

## Original styles with neutral names
FluxOS recreates the feel of other systems with original assets and neutral names; never name the other product in theme names. "Make it feel like macOS" maps to the `glass-dock` style (floating translucent dock, global menu bar, rounded corners, smooth motion) with Inter type. "Like Windows 11" maps to `snap-layouts` (bottom taskbar, snap zones, mica-like blur). "Tiling like i3 or Hyprland" maps to `tiling`. Use `desktop.apply_style`, then adjust with `desktop.set_layout` or `desktop.set_windows`.

## Colors and contrast
Prefer `design.generate_palette` with one brand or accent color: it derives background, surfaces, text and status colors and guarantees WCAG AA contrast (AAA with `contrast: high`). Use `design.set_palette` only for individual overrides. Text must stay at 4.5:1 against the background, or 7:1 when high contrast is on. Brand colors given as hex codes should be passed through unchanged.

## Typography
Only open-licensed fonts from the catalog are allowed: Inter, IBM Plex Sans and Mono, JetBrains Mono, Lexend, Atkinson Hyperlegible Next, Noto Sans, Fira Sans and Code, Source Sans 3, Open Sans, Roboto, Rubik and Orbitron. Company brand fonts must be supplied as licensed files and are not installed by the agent.

## Motion and effects
Motion presets are snappy, smooth, calm, minimal and none. Glass and frosted looks raise `design.set_effects` blur and transparency; dense layouts lower them. `accessibility.set` with `reduce_motion` disables animation everywhere.

## Wallpapers
Wallpapers are generated SVG art colored from the palette: aurora, gradient, mesh, grid or solid. A new `seed` gives a new variation of the same style.
