//! Theme exporters: design tokens to CSS, GTK, KDE/Qt, Hyprland, GNOME dconf
//! and a Plasma layout script. Native toolkits differ (GTK, Qt and Electron
//! read different files), so each gets its own exporter.

use std::fmt::Write;

use crate::color::{self, Rgb};
use crate::spec::*;
use crate::tokens::ColorScheme;

/// Text for a CSS comment: a `*/` would end the comment early.
fn comment_safe(s: &str) -> String {
    s.replace("*/", "* /").replace(['\n', '\r'], " ")
}

fn rgb(hex: &str) -> Rgb {
    Rgb::from_hex(hex).unwrap_or(color::BLACK)
}

fn kde_rgb(hex: &str) -> String {
    let (r, g, b) = rgb(hex).to_u8();
    format!("{r},{g},{b}")
}

fn mix_hex(a: &str, b: &str, t: f64) -> String {
    rgb(a).mix(rgb(b), t).to_hex()
}

/// CSS custom properties for web shells, Electron apps and the FluxOS UI.
pub fn css(spec: &SystemSpec, header: &str) -> String {
    let t = &spec.design.tokens;
    let a = &spec.accessibility;
    let mut s = String::new();
    let _ = writeln!(s, "/* {} */", comment_safe(header));
    let _ = writeln!(s, ":root {{");
    let _ = writeln!(s, "  color-scheme: {};", if t.scheme == ColorScheme::Dark { "dark" } else { "light" });
    for (name, hex) in t.palette.entries() {
        let _ = writeln!(s, "  --flux-color-{}: {hex};", name.replace('_', "-"));
    }
    let (r, g, b) = rgb(&t.palette.surface).to_u8();
    let _ = writeln!(s, "  --flux-color-glass: rgb({r} {g} {b} / {:.3});", 1.0 - t.effects.transparency);
    let ty = &t.typography;
    let _ = writeln!(s, "  --flux-font-sans: \"{}\", ui-sans-serif, system-ui, sans-serif;", ty.font_sans);
    let _ = writeln!(s, "  --flux-font-mono: \"{}\", ui-monospace, monospace;", ty.font_mono);
    let _ = writeln!(s, "  --flux-font-display: \"{}\", \"{}\", sans-serif;", ty.font_display, ty.font_sans);
    let _ = writeln!(s, "  --flux-font-weight-regular: {};", ty.weight_regular);
    let _ = writeln!(s, "  --flux-font-weight-bold: {};", ty.weight_bold);
    let _ = writeln!(s, "  --flux-line-height: {};", ty.line_height);
    for (name, px) in t.type_scale(a.effective_text_scale()) {
        let _ = writeln!(s, "  --flux-font-size-{name}: {px}px;");
    }
    for (name, px) in t.spacing_scale() {
        let _ = writeln!(s, "  --flux-space-{name}: {px}px;");
    }
    let _ = writeln!(s, "  --flux-radius-sm: {}px;", t.shape.radius_sm_px);
    let _ = writeln!(s, "  --flux-radius-md: {}px;", t.shape.radius_md_px);
    let _ = writeln!(s, "  --flux-radius-lg: {}px;", t.shape.radius_lg_px);
    let _ = writeln!(s, "  --flux-border-width: {}px;", t.shape.border_width_px);
    for (name, ms) in t.effective_durations(a.reduce_motion) {
        let _ = writeln!(s, "  --flux-duration-{name}: {ms}ms;");
    }
    let [x1, y1, x2, y2] = t.motion.easing;
    let _ = writeln!(s, "  --flux-easing: cubic-bezier({x1}, {y1}, {x2}, {y2});");
    let _ = writeln!(s, "  --flux-blur: {}px;", t.effects.blur_px);
    let _ = writeln!(s, "  --flux-shadow-raised: 0 8px 24px rgb(0 0 0 / {:.2});", t.effects.shadow_opacity);
    let _ = writeln!(s, "  --flux-window-radius: {}px;", spec.desktop.windows.corner_radius_px);
    let _ = writeln!(s, "  --flux-window-gap: {}px;", spec.desktop.windows.gaps_px);
    let _ = writeln!(s, "  --flux-dock-size: {}px;", spec.desktop.dock.size_px);
    let _ = writeln!(s, "  --flux-panel-height: {}px;", spec.desktop.panel.height_px);
    let _ = writeln!(s, "  --flux-text-scale: {};", a.effective_text_scale());
    let _ = writeln!(s, "}}");
    let _ = writeln!(s, "@media (prefers-reduced-motion: reduce) {{");
    let _ =
        writeln!(s, "  :root {{ --flux-duration-fast: 0ms; --flux-duration-base: 0ms; --flux-duration-slow: 0ms; }}");
    let _ = writeln!(s, "}}");
    s
}

/// GTK 3/4 and libadwaita named colors (both `@define-color` and CSS variables).
pub fn gtk_css(spec: &SystemSpec, header: &str) -> String {
    let p = &spec.design.tokens.palette;
    let pairs: Vec<(&str, String)> = vec![
        ("accent_color", p.accent.clone()),
        ("accent_bg_color", p.accent.clone()),
        ("accent_fg_color", p.accent_text.clone()),
        ("window_bg_color", p.background.clone()),
        ("window_fg_color", p.text.clone()),
        ("view_bg_color", p.surface.clone()),
        ("view_fg_color", p.text.clone()),
        ("headerbar_bg_color", p.surface.clone()),
        ("headerbar_fg_color", p.text.clone()),
        ("headerbar_border_color", p.border.clone()),
        ("sidebar_bg_color", p.surface.clone()),
        ("sidebar_fg_color", p.text.clone()),
        ("card_bg_color", p.surface_raised.clone()),
        ("card_fg_color", p.text.clone()),
        ("dialog_bg_color", p.surface_raised.clone()),
        ("dialog_fg_color", p.text.clone()),
        ("popover_bg_color", p.surface_raised.clone()),
        ("popover_fg_color", p.text.clone()),
        ("destructive_bg_color", p.danger.clone()),
        ("destructive_fg_color", color::best_text_on(rgb(&p.danger)).to_hex()),
        ("success_color", p.success.clone()),
        ("warning_color", p.warning.clone()),
        ("error_color", p.danger.clone()),
        ("borders", p.border.clone()),
    ];
    let mut s = format!("/* {} */\n", comment_safe(header));
    for (name, hex) in &pairs {
        let _ = writeln!(s, "@define-color {name} {hex};");
    }
    let _ = writeln!(s, ":root {{");
    for (name, hex) in &pairs {
        let _ = writeln!(s, "  --{}: {hex};", name.replace('_', "-"));
    }
    let _ = writeln!(s, "  --window-radius: {}px;", spec.desktop.windows.corner_radius_px);
    let _ = writeln!(s, "}}");
    s
}

/// `settings.ini` for GTK 3 and 4 (system-wide defaults in /etc/xdg).
pub fn gtk_settings_ini(spec: &SystemSpec) -> String {
    let ty = &spec.design.tokens.typography;
    let pt = px_to_pt(ty.base_size_px as f64 * spec.accessibility.effective_text_scale());
    format!(
        "[Settings]\ngtk-application-prefer-dark-theme={}\ngtk-icon-theme-name={}\ngtk-cursor-theme-name={}\ngtk-font-name={} {}\ngtk-enable-animations={}\n",
        u8::from(spec.design.tokens.scheme == ColorScheme::Dark),
        spec.design.icon_theme,
        spec.design.cursor_theme,
        ty.font_sans,
        pt,
        spec.design.tokens.motion.animation_scale > 0.0 && !spec.accessibility.reduce_motion,
    )
}

pub fn px_to_pt(px: f64) -> u32 {
    (px * 0.75).round().max(7.0) as u32
}

/// KDE color scheme (`share/color-schemes/FluxOS.colors`), also usable as
/// the `[Colors:*]` part of kdeglobals.
pub fn kde_colors(spec: &SystemSpec) -> String {
    let p = &spec.design.tokens.palette;
    let alt = mix_hex(&p.surface, &p.text, 0.04);
    let inactive = p.text_muted.clone();
    let mut s = String::new();
    let section = |s: &mut String, name: &str, bg: &str, bg_alt: &str, fg: &str| {
        let _ = writeln!(s, "[Colors:{name}]");
        let _ = writeln!(s, "BackgroundNormal={}", kde_rgb(bg));
        let _ = writeln!(s, "BackgroundAlternate={}", kde_rgb(bg_alt));
        let _ = writeln!(s, "ForegroundNormal={}", kde_rgb(fg));
        let _ = writeln!(s, "ForegroundInactive={}", kde_rgb(&inactive));
        let _ = writeln!(s, "ForegroundActive={}", kde_rgb(&p.accent));
        let _ = writeln!(s, "ForegroundLink={}", kde_rgb(&p.accent));
        let _ = writeln!(s, "ForegroundVisited={}", kde_rgb(&p.secondary));
        let _ = writeln!(s, "ForegroundNegative={}", kde_rgb(&p.danger));
        let _ = writeln!(s, "ForegroundNeutral={}", kde_rgb(&p.warning));
        let _ = writeln!(s, "ForegroundPositive={}", kde_rgb(&p.success));
        let _ = writeln!(s, "DecorationFocus={}", kde_rgb(&p.accent));
        let _ = writeln!(s, "DecorationHover={}", kde_rgb(&p.accent));
        let _ = writeln!(s);
    };
    section(&mut s, "Window", &p.background, &alt, &p.text);
    section(&mut s, "View", &p.surface, &alt, &p.text);
    section(&mut s, "Button", &p.surface_raised, &p.surface, &p.text);
    section(&mut s, "Header", &p.surface, &p.background, &p.text);
    section(&mut s, "Tooltip", &p.surface_raised, &p.surface, &p.text);
    section(&mut s, "Complementary", &p.background, &p.surface, &p.text);
    section(&mut s, "Selection", &p.accent, &p.accent, &p.accent_text);
    let _ = writeln!(s, "[WM]");
    let _ = writeln!(s, "activeBackground={}", kde_rgb(&p.surface));
    let _ = writeln!(s, "activeForeground={}", kde_rgb(&p.text));
    let _ = writeln!(s, "inactiveBackground={}", kde_rgb(&p.background));
    let _ = writeln!(s, "inactiveForeground={}", kde_rgb(&p.text_muted));
    let _ = writeln!(s, "activeBlend={}", kde_rgb(&p.accent));
    let _ = writeln!(s, "inactiveBlend={}", kde_rgb(&p.border));
    s
}

pub fn kde_scheme_file(spec: &SystemSpec) -> String {
    format!("[General]\nColorScheme=FluxOS\nName={}\n\n{}", spec.design.theme_name, kde_colors(spec))
}

/// System-wide Plasma defaults (`/etc/xdg/kdeglobals`).
pub fn kdeglobals(spec: &SystemSpec) -> String {
    let ty = &spec.design.tokens.typography;
    let scale = spec.accessibility.effective_text_scale();
    let pt = px_to_pt(ty.base_size_px as f64 * scale);
    let small = px_to_pt((ty.base_size_px as f64 - 2.0) * scale);
    let font = |fam: &str, size: u32| format!("{fam},{size},-1,5,{},0,0,0,0,0", ty.weight_regular.min(1000));
    let anim = if spec.accessibility.reduce_motion { 0.0 } else { spec.design.tokens.motion.animation_scale };
    let laf = if spec.design.tokens.scheme == ColorScheme::Dark {
        "org.kde.breezedark.desktop"
    } else {
        "org.kde.breeze.desktop"
    };
    format!(
        "[General]\nColorScheme=FluxOS\nfont={}\nfixed={}\nsmallestReadableFont={}\nmenuFont={}\ntoolBarFont={}\n\n[WM]\nactiveFont={}\n\n[Icons]\nTheme={}\n\n[KDE]\nLookAndFeelPackage={}\nAnimationDurationFactor={}\nwidgetStyle=Breeze\n\n{}",
        font(&ty.font_sans, pt),
        font(&ty.font_mono, pt),
        font(&ty.font_sans, small),
        font(&ty.font_sans, pt),
        font(&ty.font_sans, small),
        font(&ty.font_display, pt),
        spec.design.icon_theme,
        laf,
        anim,
        kde_colors(spec)
    )
}

/// KWin defaults: effects, tiling/snapping scripts, virtual desktops.
pub fn kwinrc(spec: &SystemSpec) -> String {
    let w = &spec.desktop.windows;
    let blur = w.blur && spec.design.tokens.effects.blur_px > 0;
    let strength = (spec.design.tokens.effects.blur_px as u32 / 4).clamp(1, 15);
    format!(
        "[Plugins]\nblurEnabled={blur}\ncontrastEnabled={blur}\nkrohnkiteEnabled={}\nkzonesEnabled={}\n\n[Effect-blur]\nBlurStrength={strength}\nNoiseStrength=2\n\n[Desktops]\nNumber={}\nRows=1\n\n[Windows]\nElectricBorderMaximize={}\nElectricBorderTiling={}\n\n[Script-krohnkite]\nscreenGapTop={g}\nscreenGapBottom={g}\nscreenGapLeft={g}\nscreenGapRight={g}\ntileLayoutGap={g}\n",
        w.tiling,
        w.snapping && !w.tiling,
        spec.desktop.workspaces,
        w.snapping,
        w.snapping,
        g = w.gaps_px,
    )
}

/// Plasma desktop-scripting layout. Plasma runs each update script once per
/// user, so the file name carries the generation to re-apply on change.
pub fn plasma_layout_js(spec: &SystemSpec, tag: &str) -> String {
    let d = &spec.desktop;
    let panel_loc = match d.panel.position {
        PanelPosition::Top => "top",
        PanelPosition::Bottom => "bottom",
    };
    let dock_loc = match d.dock.position {
        DockPosition::Bottom => "bottom",
        DockPosition::Left => "left",
        DockPosition::Right => "right",
    };
    let mut s = String::new();
    let _ = writeln!(s, "// FluxOS layout {tag}. Generated by fluxd; rebuilt on every generation.");
    let _ = writeln!(s, "var existing = panels();");
    let _ = writeln!(s, "for (var i = 0; i < existing.length; i++) {{ existing[i].remove(); }}");
    let _ = writeln!(s, "var bar = new Panel;");
    let _ = writeln!(s, "bar.location = \"{panel_loc}\";");
    let _ = writeln!(s, "bar.height = {};", d.panel.height_px);
    let _ = writeln!(s, "bar.floating = false;");
    let _ = writeln!(s, "bar.addWidget(\"org.kde.plasma.kickoff\");");
    if d.panel.global_menu {
        let _ = writeln!(s, "bar.addWidget(\"org.kde.plasma.appmenu\");");
    }
    if !d.dock.enabled {
        let _ = writeln!(s, "bar.addWidget(\"org.kde.plasma.icontasks\");");
    }
    let _ = writeln!(s, "bar.addWidget(\"org.kde.plasma.panelspacer\");");
    let _ = writeln!(s, "bar.addWidget(\"org.kde.plasma.systemtray\");");
    let _ = writeln!(s, "bar.addWidget(\"org.kde.plasma.digitalclock\");");
    if d.dock.enabled && !(d.dock.position == DockPosition::Bottom && d.panel.position == PanelPosition::Bottom) {
        let _ = writeln!(s, "var dock = new Panel;");
        let _ = writeln!(s, "dock.location = \"{dock_loc}\";");
        let _ = writeln!(s, "dock.height = {};", d.dock.size_px as u32 + 12);
        let _ = writeln!(s, "dock.floating = {};", d.dock.floating);
        let _ = writeln!(s, "dock.lengthMode = \"fit\";");
        let _ = writeln!(s, "dock.alignment = \"center\";");
        let _ = writeln!(s, "dock.hiding = \"{}\";", if d.dock.autohide { "autohide" } else { "dodgewindows" });
        let _ = writeln!(s, "dock.addWidget(\"org.kde.plasma.icontasks\");");
    }
    let _ = writeln!(s, "var desks = desktops();");
    let _ = writeln!(s, "for (var j = 0; j < desks.length; j++) {{");
    let _ = writeln!(s, "  desks[j].wallpaperPlugin = \"org.kde.image\";");
    let _ = writeln!(s, "  desks[j].currentConfigGroup = [\"Wallpaper\", \"org.kde.image\", \"General\"];");
    let _ = writeln!(s, "  desks[j].writeConfig(\"Image\", \"file:///etc/fluxos/wallpaper.svg\");");
    let _ = writeln!(s, "}}");
    s
}

/// Hyprland snippet; include it with `source = /etc/xdg/hypr/fluxos.conf`.
pub fn hyprland(spec: &SystemSpec, header: &str) -> String {
    let t = &spec.design.tokens;
    let w = &spec.desktop.windows;
    let hex = |h: &str, a: &str| format!("rgba({}{a})", h.trim_start_matches('#'));
    let [x1, y1, x2, y2] = t.motion.easing;
    let anim_on = t.motion.animation_scale > 0.0 && !spec.accessibility.reduce_motion;
    let ds = |ms: u16| ((ms as f64 * t.motion.animation_scale) / 100.0).round().max(1.0) as u32;
    let blur = w.blur && t.effects.blur_px > 0;
    format!(
        "# {header}\n\
         general {{\n    gaps_in = {gi}\n    gaps_out = {go}\n    border_size = {bw}\n    col.active_border = {ab}\n    col.inactive_border = {ib}\n    layout = dwindle\n}}\n\
         decoration {{\n    rounding = {r}\n    inactive_opacity = {io:.2}\n    blur {{\n        enabled = {blur}\n        size = {bs}\n        passes = 2\n    }}\n}}\n\
         animations {{\n    enabled = {anim_on}\n    bezier = flux, {x1}, {y1}, {x2}, {y2}\n    animation = windows, 1, {d1}, flux\n    animation = fade, 1, {d0}, flux\n    animation = workspaces, 1, {d2}, flux\n}}\n\
         misc {{\n    background_color = {bg}\n}}\n",
        gi = (w.gaps_px / 2).max(if w.tiling { 2 } else { 0 }),
        go = w.gaps_px,
        bw = t.shape.border_width_px.max(1),
        ab = hex(&t.palette.accent, "ee"),
        ib = hex(&t.palette.border, "aa"),
        r = w.corner_radius_px,
        io = 1.0 - t.effects.transparency * 0.5,
        bs = (t.effects.blur_px / 3).max(1),
        d0 = ds(t.motion.duration_fast_ms),
        d1 = ds(t.motion.duration_base_ms),
        d2 = ds(t.motion.duration_slow_ms),
        bg = hex(&t.palette.background, "ff"),
    )
}

/// A GNOME dconf value with its GVariant type.
#[derive(Debug, Clone, PartialEq)]
pub enum Dconf {
    Str(String),
    Bool(bool),
    Int(i32),
    Uint(u32),
    Double(f64),
    StrList(Vec<String>),
}

/// GNOME settings as (dconf directory, [(key, value)]).
pub fn gnome_dconf(spec: &SystemSpec) -> Vec<(String, Vec<(String, Dconf)>)> {
    let t = &spec.design.tokens;
    let a = &spec.accessibility;
    let d = &spec.desktop;
    let ty = &t.typography;
    let pt = px_to_pt(ty.base_size_px as f64);
    let k = |s: &str| s.to_string();
    let mut extensions = vec![k("appindicatorsupport@rgcjonas.gmail.com")];
    if d.dock.enabled {
        extensions.push(k("dash-to-dock@micxgx.gmail.com"));
    }
    if d.windows.blur && t.effects.blur_px > 0 {
        extensions.push(k("blur-my-shell@aunetx"));
    }
    if d.windows.tiling {
        extensions.push(k("forge@jmmaranan.com"));
    }
    let dock_pos = match d.dock.position {
        DockPosition::Bottom => "BOTTOM",
        DockPosition::Left => "LEFT",
        DockPosition::Right => "RIGHT",
    };
    let wallpaper = k("file:///etc/fluxos/wallpaper.svg");
    vec![
        (
            k("org/gnome/desktop/interface"),
            vec![
                (
                    k("color-scheme"),
                    Dconf::Str(if t.scheme == ColorScheme::Dark { k("prefer-dark") } else { k("default") }),
                ),
                (k("accent-color"), Dconf::Str(k(color::gnome_accent_name(rgb(&t.palette.accent))))),
                (k("font-name"), Dconf::Str(format!("{} {pt}", ty.font_sans))),
                (k("document-font-name"), Dconf::Str(format!("{} {pt}", ty.font_sans))),
                (k("monospace-font-name"), Dconf::Str(format!("{} {pt}", ty.font_mono))),
                (k("icon-theme"), Dconf::Str(spec.design.icon_theme.clone())),
                (k("cursor-theme"), Dconf::Str(spec.design.cursor_theme.clone())),
                (k("text-scaling-factor"), Dconf::Double(a.effective_text_scale())),
                (k("enable-animations"), Dconf::Bool(t.motion.animation_scale > 0.0 && !a.reduce_motion)),
            ],
        ),
        (k("org/gnome/desktop/wm/preferences"), vec![(k("num-workspaces"), Dconf::Int(d.workspaces as i32))]),
        (
            k("org/gnome/mutter"),
            vec![(k("edge-tiling"), Dconf::Bool(d.windows.snapping)), (k("dynamic-workspaces"), Dconf::Bool(false))],
        ),
        (k("org/gnome/shell"), vec![(k("enabled-extensions"), Dconf::StrList(extensions))]),
        (
            k("org/gnome/shell/extensions/dash-to-dock"),
            vec![
                (k("dock-position"), Dconf::Str(k(dock_pos))),
                (k("dock-fixed"), Dconf::Bool(!d.dock.autohide)),
                (k("autohide"), Dconf::Bool(d.dock.autohide)),
                (k("intellihide"), Dconf::Bool(d.dock.autohide)),
                (k("dash-max-icon-size"), Dconf::Int(d.dock.size_px as i32)),
                (k("extend-height"), Dconf::Bool(!d.dock.floating)),
            ],
        ),
        (
            k("org/gnome/desktop/session"),
            vec![(
                k("idle-delay"),
                Dconf::Uint(if spec.security.screen_lock.enabled {
                    spec.security.screen_lock.idle_minutes as u32 * 60
                } else {
                    0
                }),
            )],
        ),
        (
            k("org/gnome/desktop/screensaver"),
            vec![
                (k("lock-enabled"), Dconf::Bool(spec.security.screen_lock.enabled)),
                (k("lock-delay"), Dconf::Uint(0)),
            ],
        ),
        (k("org/gnome/desktop/a11y/applications"), vec![(k("screen-reader-enabled"), Dconf::Bool(a.screen_reader))]),
        (k("org/gnome/desktop/a11y/interface"), vec![(k("high-contrast"), Dconf::Bool(a.high_contrast))]),
        (
            k("org/gnome/desktop/background"),
            vec![
                (k("picture-uri"), Dconf::Str(wallpaper.clone())),
                (k("picture-uri-dark"), Dconf::Str(wallpaper)),
                (k("picture-options"), Dconf::Str(k("zoom"))),
            ],
        ),
    ]
}

/// dconf keyfile rendering (for non-Nix targets and previews).
pub fn dconf_keyfile(spec: &SystemSpec) -> String {
    let mut s = String::new();
    for (dir, entries) in gnome_dconf(spec) {
        let _ = writeln!(s, "[{dir}]");
        for (key, value) in entries {
            let v = match value {
                Dconf::Str(v) => format!("'{}'", v.replace('\'', "\\'")),
                Dconf::Bool(v) => v.to_string(),
                Dconf::Int(v) => v.to_string(),
                Dconf::Uint(v) => format!("uint32 {v}"),
                Dconf::Double(v) => format!("{v:?}"),
                Dconf::StrList(v) => format!("[{}]", v.iter().map(|x| format!("'{x}'")).collect::<Vec<_>>().join(", ")),
            };
            let _ = writeln!(s, "{key}={v}");
        }
        let _ = writeln!(s);
    }
    s
}
