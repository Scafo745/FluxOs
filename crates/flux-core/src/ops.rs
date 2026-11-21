//! Typed operations ("tools") and how each one changes a [`SystemSpec`].
//!
//! Operations only describe intent. They never run commands; the applier
//! renders the resulting spec deterministically.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{self, NixInstall};
use crate::color::{self, Rgb};
use crate::policy::Issue;
use crate::profiles::ProfileSet;
use crate::spec::*;
use crate::tokens::{ColorScheme, ContrastLevel, generate_palette};

/// What an operation may look up while applying.
pub struct OpContext<'a> {
    pub profiles: &'a ProfileSet,
    /// Returns the spec of a stored generation.
    pub generation: &'a dyn Fn(u64) -> Option<SystemSpec>,
}

// ---------------------------------------------------------------- tier 0 --

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetPalette {
    /// Switching scheme without explicit colors regenerates the palette from the current accent.
    pub scheme: Option<ColorScheme>,
    pub background: Option<String>,
    pub surface: Option<String>,
    pub surface_raised: Option<String>,
    pub border: Option<String>,
    pub text: Option<String>,
    pub text_muted: Option<String>,
    pub accent: Option<String>,
    pub accent_text: Option<String>,
    pub secondary: Option<String>,
    pub success: Option<String>,
    pub warning: Option<String>,
    pub danger: Option<String>,
    pub theme_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GeneratePalette {
    /// Brand or accent color as #rrggbb. The full palette is derived from it with guaranteed contrast.
    pub accent: String,
    /// Optional second brand color as #rrggbb.
    pub secondary: Option<String>,
    /// Defaults to the current scheme.
    pub scheme: Option<ColorScheme>,
    /// `high` targets WCAG AAA text contrast. Defaults to high when accessibility.high_contrast is on.
    pub contrast: Option<ContrastLevel>,
    /// Display name for the theme, e.g. "Neon Arcade".
    pub theme_name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetTypography {
    /// Font family from the font catalog.
    pub font_sans: Option<String>,
    pub font_mono: Option<String>,
    pub font_display: Option<String>,
    /// 11-24
    pub base_size_px: Option<u8>,
    /// 1.05-1.5
    pub scale_ratio: Option<f64>,
    /// 1.1-2.0
    pub line_height: Option<f64>,
    pub weight_regular: Option<u16>,
    pub weight_bold: Option<u16>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetShape {
    pub radius_sm_px: Option<u8>,
    pub radius_md_px: Option<u8>,
    pub radius_lg_px: Option<u8>,
    pub spacing_unit_px: Option<u8>,
    pub border_width_px: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MotionPreset {
    Snappy,
    Smooth,
    Calm,
    Minimal,
    None,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetMotion {
    /// Starting point; explicit fields override it.
    pub preset: Option<MotionPreset>,
    pub duration_fast_ms: Option<u16>,
    pub duration_base_ms: Option<u16>,
    pub duration_slow_ms: Option<u16>,
    /// [x1, y1, x2, y2] with x in 0..1
    pub easing: Option<[f64; 4]>,
    /// 0 disables animation, 1 normal, up to 3
    pub animation_scale: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetEffects {
    pub blur_px: Option<u8>,
    pub transparency: Option<f64>,
    pub shadow_opacity: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetIcons {
    pub icon_theme: Option<String>,
    pub cursor_theme: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetWallpaper {
    pub style: WallpaperStyle,
    /// Change the seed to get a new variation of the same style.
    pub seed: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetLayout {
    pub dock_enabled: Option<bool>,
    pub dock_position: Option<DockPosition>,
    pub dock_autohide: Option<bool>,
    pub dock_size_px: Option<u8>,
    pub dock_floating: Option<bool>,
    pub panel_position: Option<PanelPosition>,
    pub global_menu: Option<bool>,
    pub panel_height_px: Option<u8>,
    pub workspaces: Option<u8>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetAccessibility {
    pub high_contrast: Option<bool>,
    pub large_text: Option<bool>,
    /// 1.0-2.0
    pub text_scale: Option<f64>,
    pub screen_reader: Option<bool>,
    pub reduce_motion: Option<bool>,
}

// ---------------------------------------------------------------- tier 1 --

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetWindows {
    pub snapping: Option<bool>,
    pub tiling: Option<bool>,
    pub gaps_px: Option<u8>,
    pub corner_radius_px: Option<u8>,
    pub blur: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplyStyle {
    pub style: StylePreset,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AppList {
    /// Catalog ids (or Flathub ids) of user-level apps.
    pub apps: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetIdentity {
    pub hostname: Option<String>,
    pub timezone: Option<String>,
    pub locale: Option<String>,
}

// ---------------------------------------------------------------- tier 2 --

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackageList {
    /// Catalog ids of system packages.
    pub packages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ServiceList {
    /// Catalog ids of services.
    pub services: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PortSet {
    pub tcp: Option<Vec<u16>>,
    pub udp: Option<Vec<u16>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KernelTune {
    /// Allowlisted sysctl keys and values.
    pub sysctl: Option<BTreeMap<String, SysctlValue>>,
    /// Keys to reset to the kernel default.
    pub unset_sysctl: Option<Vec<String>>,
    pub cpu_governor: Option<CpuGovernor>,
    pub io_scheduler: Option<IoScheduler>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetScheduler {
    pub scheduler: SchedExt,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetEnvironment {
    pub environment: DesktopEnv,
}

// ---------------------------------------------------------------- tier 3 --

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectKernel {
    pub variant: KernelVariant,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigureDrivers {
    pub gpu: Option<GpuDriver>,
    pub gaming_controllers: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigureSecurity {
    pub mac: Option<MacPolicy>,
    pub usb_storage: Option<UsbStorage>,
    pub screen_lock_enabled: Option<bool>,
    /// 1-240
    pub screen_lock_idle_minutes: Option<u16>,
    pub audit: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigureSession {
    pub mode: Option<SessionMode>,
    /// Required for kiosk and exam modes. https URLs only.
    pub kiosk_url: Option<String>,
    pub ephemeral_home: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigureBoot {
    pub splash: Option<bool>,
    pub splash_theme: Option<String>,
    pub loader_timeout_s: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplyProfile {
    /// Profile id, e.g. `gaming`, `education`, `telecom`.
    pub profile: String,
    /// Sections to take from the profile. Defaults to everything except `base`.
    pub sections: Option<Vec<Section>>,
    /// Keep currently installed packages, apps, services and open ports (default true).
    pub keep_existing: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RestoreGeneration {
    /// Generation number to restore (see GET /v1/generations).
    pub generation: u64,
}

/// A parsed, typed operation.
#[derive(Debug, Clone)]
pub enum Op {
    SetPalette(SetPalette),
    GeneratePalette(GeneratePalette),
    SetTypography(SetTypography),
    SetShape(SetShape),
    SetMotion(SetMotion),
    SetEffects(SetEffects),
    SetIcons(SetIcons),
    SetWallpaper(SetWallpaper),
    SetLayout(SetLayout),
    SetAccessibility(SetAccessibility),
    SetWindows(SetWindows),
    ApplyStyle(ApplyStyle),
    AppsInstall(AppList),
    AppsRemove(AppList),
    SetIdentity(SetIdentity),
    PackagesInstall(PackageList),
    PackagesRemove(PackageList),
    ServicesEnable(ServiceList),
    ServicesDisable(ServiceList),
    FirewallAllow(PortSet),
    FirewallRevoke(PortSet),
    KernelTune(KernelTune),
    SetScheduler(SetScheduler),
    SetEnvironment(SetEnvironment),
    SelectKernel(SelectKernel),
    ConfigureDrivers(ConfigureDrivers),
    ConfigureSecurity(ConfigureSecurity),
    ConfigureSession(ConfigureSession),
    ConfigureBoot(ConfigureBoot),
    ApplyProfile(ApplyProfile),
    RestoreGeneration(RestoreGeneration),
}

fn set<T: Clone>(slot: &mut T, v: &Option<T>) {
    if let Some(v) = v {
        *slot = v.clone();
    }
}

/// Normalizes valid colors; leaves invalid ones for the validator to report.
fn set_color(slot: &mut String, v: &Option<String>) {
    if let Some(v) = v {
        *slot = color::normalize_hex(v).unwrap_or_else(|| v.clone());
    }
}

pub fn apply_motion_preset(m: &mut crate::tokens::Motion, p: MotionPreset) {
    let (f, b, s, e, scale) = match p {
        MotionPreset::Snappy => (90, 160, 260, [0.3, 0.0, 0.0, 1.0], 1.0),
        MotionPreset::Smooth => (160, 260, 420, [0.32, 0.72, 0.0, 1.0], 1.0),
        MotionPreset::Calm => (200, 320, 520, [0.4, 0.0, 0.2, 1.0], 1.0),
        MotionPreset::Minimal => (60, 100, 160, [0.2, 0.0, 0.0, 1.0], 0.6),
        MotionPreset::None => (m.duration_fast_ms, m.duration_base_ms, m.duration_slow_ms, m.easing, 0.0),
    };
    m.duration_fast_ms = f;
    m.duration_base_ms = b;
    m.duration_slow_ms = s;
    m.easing = e;
    m.animation_scale = scale;
}

/// Applies a look-and-behavior preset to desktop layout and design tokens.
pub fn apply_style_preset(spec: &mut SystemSpec, style: StylePreset) {
    let d = &mut spec.desktop;
    let t = &mut spec.design.tokens;
    d.style = style;
    match style {
        StylePreset::GlassDock => {
            d.dock = DockSpec {
                enabled: true,
                position: DockPosition::Bottom,
                autohide: false,
                size_px: 52,
                floating: true,
            };
            d.panel = PanelSpec { position: PanelPosition::Top, global_menu: true, height_px: 28 };
            d.windows = WindowSpec { snapping: true, tiling: false, gaps_px: 8, corner_radius_px: 12, blur: true };
            t.effects.blur_px = 28;
            t.effects.transparency = 0.24;
            (t.shape.radius_sm_px, t.shape.radius_md_px, t.shape.radius_lg_px) = (8, 12, 18);
            apply_motion_preset(&mut t.motion, MotionPreset::Smooth);
        }
        StylePreset::SnapLayouts => {
            d.dock.enabled = false;
            d.panel = PanelSpec { position: PanelPosition::Bottom, global_menu: false, height_px: 44 };
            d.windows = WindowSpec { snapping: true, tiling: false, gaps_px: 6, corner_radius_px: 8, blur: true };
            t.effects.blur_px = 32;
            t.effects.transparency = 0.15;
            (t.shape.radius_sm_px, t.shape.radius_md_px, t.shape.radius_lg_px) = (4, 8, 12);
            apply_motion_preset(&mut t.motion, MotionPreset::Snappy);
        }
        StylePreset::Tiling => {
            d.dock.enabled = false;
            d.panel = PanelSpec { position: PanelPosition::Top, global_menu: false, height_px: 26 };
            d.windows = WindowSpec { snapping: false, tiling: true, gaps_px: 10, corner_radius_px: 8, blur: false };
            t.effects.blur_px = 0;
            t.effects.transparency = 0.0;
            (t.shape.radius_sm_px, t.shape.radius_md_px, t.shape.radius_lg_px) = (4, 6, 10);
            apply_motion_preset(&mut t.motion, MotionPreset::Snappy);
        }
        StylePreset::ClassicPanel => {
            d.dock.enabled = false;
            d.panel = PanelSpec { position: PanelPosition::Bottom, global_menu: false, height_px: 40 };
            d.windows = WindowSpec { snapping: true, tiling: false, gaps_px: 0, corner_radius_px: 4, blur: false };
            t.effects.blur_px = 0;
            t.effects.transparency = 0.0;
            (t.shape.radius_sm_px, t.shape.radius_md_px, t.shape.radius_lg_px) = (3, 4, 6);
            apply_motion_preset(&mut t.motion, MotionPreset::Minimal);
        }
        StylePreset::OpsCenter => {
            d.dock.enabled = false;
            d.panel = PanelSpec { position: PanelPosition::Top, global_menu: false, height_px: 24 };
            d.windows = WindowSpec { snapping: false, tiling: true, gaps_px: 4, corner_radius_px: 2, blur: false };
            d.workspaces = d.workspaces.max(8);
            t.effects.blur_px = 0;
            t.effects.transparency = 0.0;
            (t.shape.radius_sm_px, t.shape.radius_md_px, t.shape.radius_lg_px) = (2, 3, 4);
            apply_motion_preset(&mut t.motion, MotionPreset::Minimal);
        }
        StylePreset::Focus => {
            d.dock =
                DockSpec { enabled: true, position: DockPosition::Bottom, autohide: true, size_px: 44, floating: true };
            d.panel = PanelSpec { position: PanelPosition::Top, global_menu: false, height_px: 28 };
            d.windows = WindowSpec { snapping: true, tiling: false, gaps_px: 8, corner_radius_px: 10, blur: false };
            t.effects.blur_px = 12;
            t.effects.transparency = 0.08;
            apply_motion_preset(&mut t.motion, MotionPreset::Calm);
        }
        StylePreset::Custom => {}
    }
}

fn err(code: &str, path: &str, msg: impl Into<String>) -> Issue {
    Issue::error(code, path, msg)
}

impl Op {
    /// Applies the operation to `spec`. Returns problems found in the
    /// operation's own inputs; the whole candidate spec is validated later.
    pub fn apply(&self, spec: &mut SystemSpec, ctx: &OpContext<'_>) -> Vec<Issue> {
        let mut issues = Vec::new();
        match self {
            Op::SetPalette(p) => {
                let pal = &mut spec.design.tokens.palette;
                let explicit = [
                    &p.background,
                    &p.surface,
                    &p.surface_raised,
                    &p.border,
                    &p.text,
                    &p.text_muted,
                    &p.accent,
                    &p.accent_text,
                    &p.secondary,
                    &p.success,
                    &p.warning,
                    &p.danger,
                ]
                .iter()
                .any(|c| c.is_some());
                if let Some(scheme) = p.scheme {
                    if scheme != spec.design.tokens.scheme && !explicit {
                        let accent = Rgb::from_hex(&pal.accent).unwrap_or(color::BLACK);
                        let secondary = Rgb::from_hex(&pal.secondary);
                        let contrast =
                            if spec.accessibility.high_contrast { ContrastLevel::High } else { ContrastLevel::Normal };
                        *pal = generate_palette(accent, secondary, scheme, contrast);
                    }
                    spec.design.tokens.scheme = scheme;
                }
                let pal = &mut spec.design.tokens.palette;
                set_color(&mut pal.background, &p.background);
                set_color(&mut pal.surface, &p.surface);
                set_color(&mut pal.surface_raised, &p.surface_raised);
                set_color(&mut pal.border, &p.border);
                set_color(&mut pal.text, &p.text);
                set_color(&mut pal.text_muted, &p.text_muted);
                set_color(&mut pal.accent, &p.accent);
                set_color(&mut pal.accent_text, &p.accent_text);
                set_color(&mut pal.secondary, &p.secondary);
                set_color(&mut pal.success, &p.success);
                set_color(&mut pal.warning, &p.warning);
                set_color(&mut pal.danger, &p.danger);
                set(&mut spec.design.theme_name, &p.theme_name);
            }
            Op::GeneratePalette(p) => {
                let Some(accent) = Rgb::from_hex(&p.accent) else {
                    issues.push(err(
                        "invalid_color",
                        "params.accent",
                        format!("{:?} is not a #rrggbb color", p.accent),
                    ));
                    return issues;
                };
                let secondary = match &p.secondary {
                    Some(s) => match Rgb::from_hex(s) {
                        Some(c) => Some(c),
                        None => {
                            issues.push(err(
                                "invalid_color",
                                "params.secondary",
                                format!("{s:?} is not a #rrggbb color"),
                            ));
                            return issues;
                        }
                    },
                    None => None,
                };
                let scheme = p.scheme.unwrap_or(spec.design.tokens.scheme);
                let contrast = p.contrast.unwrap_or(if spec.accessibility.high_contrast {
                    ContrastLevel::High
                } else {
                    ContrastLevel::Normal
                });
                spec.design.tokens.scheme = scheme;
                spec.design.tokens.palette = generate_palette(accent, secondary, scheme, contrast);
                set(&mut spec.design.theme_name, &p.theme_name);
            }
            Op::SetTypography(p) => {
                let t = &mut spec.design.tokens.typography;
                set(&mut t.font_sans, &p.font_sans);
                set(&mut t.font_mono, &p.font_mono);
                set(&mut t.font_display, &p.font_display);
                set(&mut t.base_size_px, &p.base_size_px);
                set(&mut t.scale_ratio, &p.scale_ratio);
                set(&mut t.line_height, &p.line_height);
                set(&mut t.weight_regular, &p.weight_regular);
                set(&mut t.weight_bold, &p.weight_bold);
            }
            Op::SetShape(p) => {
                let s = &mut spec.design.tokens.shape;
                set(&mut s.radius_sm_px, &p.radius_sm_px);
                set(&mut s.radius_md_px, &p.radius_md_px);
                set(&mut s.radius_lg_px, &p.radius_lg_px);
                set(&mut s.spacing_unit_px, &p.spacing_unit_px);
                set(&mut s.border_width_px, &p.border_width_px);
            }
            Op::SetMotion(p) => {
                let m = &mut spec.design.tokens.motion;
                if let Some(preset) = p.preset {
                    apply_motion_preset(m, preset);
                }
                set(&mut m.duration_fast_ms, &p.duration_fast_ms);
                set(&mut m.duration_base_ms, &p.duration_base_ms);
                set(&mut m.duration_slow_ms, &p.duration_slow_ms);
                set(&mut m.easing, &p.easing);
                set(&mut m.animation_scale, &p.animation_scale);
            }
            Op::SetEffects(p) => {
                let e = &mut spec.design.tokens.effects;
                set(&mut e.blur_px, &p.blur_px);
                set(&mut e.transparency, &p.transparency);
                set(&mut e.shadow_opacity, &p.shadow_opacity);
            }
            Op::SetIcons(p) => {
                set(&mut spec.design.icon_theme, &p.icon_theme);
                set(&mut spec.design.cursor_theme, &p.cursor_theme);
            }
            Op::SetWallpaper(p) => {
                spec.desktop.wallpaper.style = p.style;
                set(&mut spec.desktop.wallpaper.seed, &p.seed);
            }
            Op::SetLayout(p) => {
                let d = &mut spec.desktop;
                set(&mut d.dock.enabled, &p.dock_enabled);
                set(&mut d.dock.position, &p.dock_position);
                set(&mut d.dock.autohide, &p.dock_autohide);
                set(&mut d.dock.size_px, &p.dock_size_px);
                set(&mut d.dock.floating, &p.dock_floating);
                set(&mut d.panel.position, &p.panel_position);
                set(&mut d.panel.global_menu, &p.global_menu);
                set(&mut d.panel.height_px, &p.panel_height_px);
                set(&mut d.workspaces, &p.workspaces);
                d.style = StylePreset::Custom;
            }
            Op::SetAccessibility(p) => {
                let a = &mut spec.accessibility;
                set(&mut a.high_contrast, &p.high_contrast);
                set(&mut a.large_text, &p.large_text);
                set(&mut a.text_scale, &p.text_scale);
                set(&mut a.screen_reader, &p.screen_reader);
                set(&mut a.reduce_motion, &p.reduce_motion);
                // Turning on high contrast regenerates the palette so it passes AAA.
                if p.high_contrast == Some(true) {
                    let pal = &spec.design.tokens.palette;
                    let accent = Rgb::from_hex(&pal.accent).unwrap_or(color::BLACK);
                    let secondary = Rgb::from_hex(&pal.secondary);
                    spec.design.tokens.palette =
                        generate_palette(accent, secondary, spec.design.tokens.scheme, ContrastLevel::High);
                }
            }
            Op::SetWindows(p) => {
                let w = &mut spec.desktop.windows;
                set(&mut w.snapping, &p.snapping);
                set(&mut w.tiling, &p.tiling);
                set(&mut w.gaps_px, &p.gaps_px);
                set(&mut w.corner_radius_px, &p.corner_radius_px);
                set(&mut w.blur, &p.blur);
                spec.desktop.style = StylePreset::Custom;
            }
            Op::ApplyStyle(p) => {
                if p.style == StylePreset::Custom {
                    issues.push(err(
                        "invalid_value",
                        "params.style",
                        "`custom` is not a preset; use desktop.set_layout instead",
                    ));
                } else {
                    apply_style_preset(spec, p.style);
                }
            }
            Op::AppsInstall(p) | Op::AppsRemove(p) => {
                let install = matches!(self, Op::AppsInstall(_));
                if p.apps.is_empty() {
                    issues.push(err("empty_list", "params.apps", "list at least one app"));
                }
                for (i, q) in p.apps.iter().enumerate() {
                    match catalog::resolve_package(q) {
                        Some(entry) if entry.flatpak.is_some() => {
                            if install {
                                spec.packages.flatpak.insert(entry.id.to_string());
                            } else {
                                spec.packages.flatpak.remove(entry.id);
                            }
                        }
                        Some(entry) => issues.push(err(
                            "no_flatpak",
                            &format!("params.apps[{i}]"),
                            format!(
                                "{} has no Flathub build; install it as a system package with packages.install",
                                entry.id
                            ),
                        )),
                        None => issues.push(
                            err(
                                "unknown_app",
                                &format!("params.apps[{i}]"),
                                format!("{q:?} is not in the app catalog"),
                            )
                            .suggest(catalog::suggest(
                                q,
                                catalog::PACKAGES.iter().filter(|p| p.flatpak.is_some()).map(|p| p.id),
                                3,
                            )),
                        ),
                    }
                }
            }
            Op::SetIdentity(p) => {
                set(&mut spec.base.hostname, &p.hostname);
                set(&mut spec.base.timezone, &p.timezone);
                set(&mut spec.base.locale, &p.locale);
            }
            Op::PackagesInstall(p) | Op::PackagesRemove(p) => {
                let install = matches!(self, Op::PackagesInstall(_));
                if p.packages.is_empty() {
                    issues.push(err("empty_list", "params.packages", "list at least one package"));
                }
                for (i, q) in p.packages.iter().enumerate() {
                    match catalog::resolve_package(q) {
                        Some(entry) if !matches!(entry.nix, NixInstall::FlatpakOnly) => {
                            if install {
                                spec.packages.system.insert(entry.id.to_string());
                            } else {
                                spec.packages.system.remove(entry.id);
                            }
                        }
                        Some(entry) => issues.push(err(
                            "flatpak_only",
                            &format!("params.packages[{i}]"),
                            format!("{} is only available from Flathub; use apps.install", entry.id),
                        )),
                        None => issues.push(
                            err(
                                "unknown_package",
                                &format!("params.packages[{i}]"),
                                format!("{q:?} is not in the package catalog"),
                            )
                            .suggest(catalog::suggest(
                                q,
                                catalog::PACKAGES.iter().map(|p| p.id),
                                3,
                            )),
                        ),
                    }
                }
            }
            Op::ServicesEnable(p) | Op::ServicesDisable(p) => {
                let enable = matches!(self, Op::ServicesEnable(_));
                if p.services.is_empty() {
                    issues.push(err("empty_list", "params.services", "list at least one service"));
                }
                for (i, q) in p.services.iter().enumerate() {
                    match catalog::service(q) {
                        Some(s) => {
                            if enable {
                                spec.services.enabled.insert(s.id.to_string());
                            } else {
                                spec.services.enabled.remove(s.id);
                            }
                        }
                        None => issues.push(
                            err(
                                "unknown_service",
                                &format!("params.services[{i}]"),
                                format!("{q:?} is not in the service catalog"),
                            )
                            .suggest(catalog::suggest(
                                q,
                                catalog::SERVICES.iter().map(|s| s.id),
                                3,
                            )),
                        ),
                    }
                }
            }
            Op::FirewallAllow(p) | Op::FirewallRevoke(p) => {
                let allow = matches!(self, Op::FirewallAllow(_));
                if p.tcp.as_ref().is_none_or(Vec::is_empty) && p.udp.as_ref().is_none_or(Vec::is_empty) {
                    issues.push(err("empty_list", "params", "give at least one tcp or udp port"));
                }
                let fw = &mut spec.security.firewall;
                for (proto, ports, set) in
                    [("tcp", &p.tcp, &mut fw.allowed_tcp_ports), ("udp", &p.udp, &mut fw.allowed_udp_ports)]
                {
                    for (i, port) in ports.iter().flatten().enumerate() {
                        if *port == 0 {
                            issues.push(err("invalid_port", &format!("params.{proto}[{i}]"), "port 0 is not valid"));
                        } else if allow {
                            set.insert(*port);
                        } else {
                            set.remove(port);
                        }
                    }
                }
            }
            Op::KernelTune(p) => {
                let k = &mut spec.kernel;
                if let Some(map) = &p.sysctl {
                    for (key, value) in map {
                        k.sysctl.insert(key.clone(), value.clone());
                    }
                }
                for key in p.unset_sysctl.iter().flatten() {
                    k.sysctl.remove(key);
                }
                set(&mut k.cpu_governor, &p.cpu_governor);
                set(&mut k.io_scheduler, &p.io_scheduler);
            }
            Op::SetScheduler(p) => spec.kernel.sched_ext = p.scheduler,
            Op::SetEnvironment(p) => spec.desktop.environment = p.environment,
            Op::SelectKernel(p) => spec.kernel.variant = p.variant,
            Op::ConfigureDrivers(p) => {
                set(&mut spec.drivers.gpu, &p.gpu);
                set(&mut spec.drivers.gaming_controllers, &p.gaming_controllers);
            }
            Op::ConfigureSecurity(p) => {
                let s = &mut spec.security;
                set(&mut s.mac, &p.mac);
                set(&mut s.usb_storage, &p.usb_storage);
                set(&mut s.screen_lock.enabled, &p.screen_lock_enabled);
                set(&mut s.screen_lock.idle_minutes, &p.screen_lock_idle_minutes);
                set(&mut s.audit, &p.audit);
            }
            Op::ConfigureSession(p) => {
                let s = &mut spec.session;
                set(&mut s.mode, &p.mode);
                if p.kiosk_url.is_some() {
                    s.kiosk_url = p.kiosk_url.clone();
                }
                set(&mut s.ephemeral_home, &p.ephemeral_home);
                if s.mode == SessionMode::Standard && p.kiosk_url.is_none() {
                    s.kiosk_url = None;
                }
            }
            Op::ConfigureBoot(p) => {
                set(&mut spec.boot.splash, &p.splash);
                set(&mut spec.boot.splash_theme, &p.splash_theme);
                set(&mut spec.boot.loader_timeout_s, &p.loader_timeout_s);
            }
            Op::ApplyProfile(p) => match ctx.profiles.get(&p.profile) {
                Some(profile) => {
                    let sections = p.sections.clone().unwrap_or_else(|| Section::PROFILE_DEFAULT.to_vec());
                    apply_profile_sections(spec, &profile.spec, &sections, p.keep_existing.unwrap_or(true));
                }
                None => issues.push(
                    err("unknown_profile", "params.profile", format!("profile {:?} does not exist", p.profile))
                        .suggest(catalog::suggest(&p.profile, ctx.profiles.ids(), 3)),
                ),
            },
            Op::RestoreGeneration(p) => match (ctx.generation)(p.generation) {
                Some(old) => *spec = old,
                None => issues.push(err(
                    "unknown_generation",
                    "params.generation",
                    format!("generation {} does not exist", p.generation),
                )),
            },
        }
        issues
    }
}

/// Copies sections from `profile` into `spec`.
pub fn apply_profile_sections(spec: &mut SystemSpec, profile: &SystemSpec, sections: &[Section], keep_existing: bool) {
    let before = spec.clone();
    for s in sections {
        match s {
            Section::Base => {
                // Identity stays with the machine; only distro/release follow the profile.
                spec.base.distro = profile.base.distro.clone();
                spec.base.release = profile.base.release.clone();
            }
            Section::Boot => spec.boot = profile.boot.clone(),
            Section::Kernel => spec.kernel = profile.kernel.clone(),
            Section::Drivers => spec.drivers = profile.drivers.clone(),
            Section::Packages => spec.packages = profile.packages.clone(),
            Section::Services => spec.services = profile.services.clone(),
            Section::Security => spec.security = profile.security.clone(),
            Section::Session => spec.session = profile.session.clone(),
            Section::Desktop => spec.desktop = profile.desktop.clone(),
            Section::Design => spec.design = profile.design.clone(),
            Section::Accessibility => {
                // Accessibility is a personal need: profiles may add features, never remove them.
                let (a, p) = (&mut spec.accessibility, &profile.accessibility);
                a.high_contrast |= p.high_contrast;
                a.large_text |= p.large_text;
                a.screen_reader |= p.screen_reader;
                a.reduce_motion |= p.reduce_motion;
                a.text_scale = a.text_scale.max(p.text_scale);
            }
        }
    }
    // A profile's palette must still satisfy the user's high-contrast need.
    if spec.accessibility.high_contrast {
        let pal = &spec.design.tokens.palette;
        if color::contrast_hex(&pal.text, &pal.background).unwrap_or(0.0) < 7.0 {
            let accent = Rgb::from_hex(&pal.accent).unwrap_or(color::BLACK);
            let secondary = Rgb::from_hex(&pal.secondary);
            spec.design.tokens.palette =
                generate_palette(accent, secondary, spec.design.tokens.scheme, ContrastLevel::High);
        }
    }
    if keep_existing {
        spec.packages.system.extend(before.packages.system);
        spec.packages.flatpak.extend(before.packages.flatpak);
        spec.services.enabled.extend(before.services.enabled);
        spec.security.firewall.allowed_tcp_ports.extend(before.security.firewall.allowed_tcp_ports);
        spec.security.firewall.allowed_udp_ports.extend(before.security.firewall.allowed_udp_ports);
    }
}
