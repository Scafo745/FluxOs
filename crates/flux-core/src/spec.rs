//! The declarative system description.
//!
//! A [`SystemSpec`] fully describes a FluxOS machine: base, boot, kernel,
//! drivers, packages, services, security, session, desktop, design and
//! accessibility. The AI never edits it directly; it proposes typed
//! operations that the policy engine applies to a copy of the current spec.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::color::Rgb;
use crate::tokens::{ColorScheme, ContrastLevel, DesignTokens, Effects, Motion, Shape, Typography, generate_palette};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SystemSpec {
    pub base: BaseSpec,
    pub boot: BootSpec,
    pub kernel: KernelSpec,
    pub drivers: DriverSpec,
    pub packages: PackageSpec,
    pub services: ServiceSpec,
    pub security: SecuritySpec,
    pub session: SessionSpec,
    pub desktop: DesktopSpec,
    pub design: DesignSpec,
    pub accessibility: AccessibilitySpec,
}

/// Top-level sections, used by `profile.apply` and the diff classifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Section {
    Base,
    Boot,
    Kernel,
    Drivers,
    Packages,
    Services,
    Security,
    Session,
    Desktop,
    Design,
    Accessibility,
}

impl Section {
    pub const PROFILE_DEFAULT: [Section; 10] = [
        Section::Boot,
        Section::Kernel,
        Section::Drivers,
        Section::Packages,
        Section::Services,
        Section::Security,
        Section::Session,
        Section::Desktop,
        Section::Design,
        Section::Accessibility,
    ];
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BaseSpec {
    /// Base distribution adapter. Only `nixos` in v1.
    pub distro: String,
    /// Distribution release, e.g. `26.05`.
    pub release: String,
    pub hostname: String,
    /// IANA time zone such as `Europe/Berlin`, or `UTC`.
    pub timezone: String,
    pub locale: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BootSpec {
    /// Show a graphical boot splash (Plymouth).
    pub splash: bool,
    /// Plymouth theme from the boot-splash catalog.
    pub splash_theme: String,
    /// Boot menu timeout in seconds (0-30). Older generations stay selectable.
    pub loader_timeout_s: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum KernelVariant {
    /// Long-term-support kernel (NixOS default).
    Lts,
    /// Latest stable kernel.
    Latest,
    /// Zen kernel tuned for desktop responsiveness.
    LowLatency,
    /// XanMod kernel tuned for throughput and gaming.
    Performance,
    /// LTS kernel plus hardening boot parameters and sysctls.
    Hardened,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CpuGovernor {
    Performance,
    Powersave,
    Schedutil,
    Ondemand,
    Conservative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum IoScheduler {
    /// Keep the kernel default.
    Auto,
    #[serde(rename = "none")]
    NoOp,
    MqDeadline,
    Bfq,
    Kyber,
}

impl IoScheduler {
    pub fn kernel_name(self) -> Option<&'static str> {
        match self {
            Self::Auto => None,
            Self::NoOp => Some("none"),
            Self::MqDeadline => Some("mq-deadline"),
            Self::Bfq => Some("bfq"),
            Self::Kyber => Some("kyber"),
        }
    }
}

/// sched_ext scheduler loaded at runtime (no reboot needed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SchedExt {
    Off,
    Lavd,
    Bpfland,
    Rusty,
    Flash,
    Cosmos,
    P2dq,
}

impl SchedExt {
    pub fn binary(self) -> Option<&'static str> {
        Some(match self {
            Self::Off => return None,
            Self::Lavd => "scx_lavd",
            Self::Bpfland => "scx_bpfland",
            Self::Rusty => "scx_rusty",
            Self::Flash => "scx_flash",
            Self::Cosmos => "scx_cosmos",
            Self::P2dq => "scx_p2dq",
        })
    }
}

/// A sysctl value: integer or string (for multi-value keys such as `tcp_rmem`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum SysctlValue {
    Int(i64),
    Text(String),
}

impl std::fmt::Display for SysctlValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Int(v) => write!(f, "{v}"),
            Self::Text(v) => write!(f, "{v}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KernelSpec {
    pub variant: KernelVariant,
    /// Allowlisted runtime kernel parameters.
    pub sysctl: BTreeMap<String, SysctlValue>,
    pub cpu_governor: CpuGovernor,
    pub io_scheduler: IoScheduler,
    pub sched_ext: SchedExt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum GpuDriver {
    /// Mesa (AMD, Intel) with kernel defaults.
    Auto,
    /// Proprietary NVIDIA driver.
    Nvidia,
    /// NVIDIA open kernel modules (Turing and newer).
    NvidiaOpen,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DriverSpec {
    pub gpu: GpuDriver,
    /// Xbox/Steam controller support (xpadneo, steam-hardware udev rules).
    pub gaming_controllers: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackageSpec {
    /// System packages (catalog ids), installed through Nix.
    pub system: BTreeSet<String>,
    /// User-level apps (catalog ids), installed from Flathub.
    pub flatpak: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ServiceSpec {
    /// Enabled services (catalog ids).
    pub enabled: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MacPolicy {
    Apparmor,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum UsbStorage {
    Allow,
    Block,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FirewallSpec {
    pub enabled: bool,
    pub allowed_tcp_ports: BTreeSet<u16>,
    pub allowed_udp_ports: BTreeSet<u16>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScreenLockSpec {
    pub enabled: bool,
    /// Idle minutes before the screen locks (1-240).
    pub idle_minutes: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SecuritySpec {
    pub firewall: FirewallSpec,
    /// Mandatory access control.
    pub mac: MacPolicy,
    pub usb_storage: UsbStorage,
    pub screen_lock: ScreenLockSpec,
    /// Kernel audit subsystem and auditd.
    pub audit: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SessionMode {
    /// Normal multi-app desktop.
    Standard,
    /// Single full-screen browser app (cage compositor).
    Kiosk,
    /// Exam lock-down: kiosk plus blocked USB storage.
    Exam,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionSpec {
    pub mode: SessionMode,
    /// URL for kiosk and exam modes (https only).
    pub kiosk_url: Option<String>,
    /// Shared account whose home is wiped on every boot (labs, clinics).
    pub ephemeral_home: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DesktopEnv {
    Plasma,
    Gnome,
    Hyprland,
    Cosmic,
}

/// Original look-and-behavior presets with neutral names (PRD 7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum StylePreset {
    /// Floating translucent dock, global menu bar, smooth motion.
    GlassDock,
    /// Bottom taskbar with snap zones.
    SnapLayouts,
    /// Keyboard-driven tiling with gaps.
    Tiling,
    /// Traditional single bottom panel.
    ClassicPanel,
    /// Dense dark operations-center layout with many workspaces.
    OpsCenter,
    /// Calm, low-distraction layout.
    Focus,
    /// Hand-tuned; no preset applied.
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DockPosition {
    Bottom,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PanelPosition {
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WallpaperStyle {
    Aurora,
    Gradient,
    Mesh,
    Grid,
    Solid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DockSpec {
    pub enabled: bool,
    pub position: DockPosition,
    pub autohide: bool,
    /// Icon size in px (24-96).
    pub size_px: u8,
    pub floating: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PanelSpec {
    pub position: PanelPosition,
    /// Application menus live in the top bar.
    pub global_menu: bool,
    /// Panel height in px (20-64).
    pub height_px: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WindowSpec {
    pub snapping: bool,
    pub tiling: bool,
    /// Gap between tiled windows in px (0-32).
    pub gaps_px: u8,
    /// Window corner radius in px (0-24).
    pub corner_radius_px: u8,
    pub blur: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WallpaperSpec {
    pub style: WallpaperStyle,
    /// Seed for the generated (original) wallpaper artwork.
    pub seed: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesktopSpec {
    pub environment: DesktopEnv,
    pub style: StylePreset,
    pub dock: DockSpec,
    pub panel: PanelSpec,
    pub windows: WindowSpec,
    /// Number of virtual desktops (1-12).
    pub workspaces: u8,
    pub wallpaper: WallpaperSpec,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesignSpec {
    /// Display name of the theme, e.g. "Flux Midnight".
    pub theme_name: String,
    pub tokens: DesignTokens,
    pub icon_theme: String,
    pub cursor_theme: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AccessibilitySpec {
    /// Enforce WCAG AAA (7:1) text contrast.
    pub high_contrast: bool,
    pub large_text: bool,
    /// Global text scale (1.0-2.0).
    pub text_scale: f64,
    pub screen_reader: bool,
    pub reduce_motion: bool,
}

impl AccessibilitySpec {
    /// Text scale after applying `large_text`.
    pub fn effective_text_scale(&self) -> f64 {
        if self.large_text { self.text_scale.max(1.25) } else { self.text_scale }
    }
}

pub const DEFAULT_ACCENT: &str = "#7c5cff";

impl Default for SystemSpec {
    fn default() -> Self {
        let accent = Rgb::from_hex(DEFAULT_ACCENT).expect("valid default accent");
        let palette = generate_palette(accent, None, ColorScheme::Dark, ContrastLevel::Normal);
        SystemSpec {
            base: BaseSpec {
                distro: "nixos".into(),
                release: "26.05".into(),
                hostname: "fluxos".into(),
                timezone: "UTC".into(),
                locale: "en_US.UTF-8".into(),
            },
            boot: BootSpec { splash: true, splash_theme: "hexagon_dots".into(), loader_timeout_s: 3 },
            kernel: KernelSpec {
                variant: KernelVariant::Lts,
                sysctl: BTreeMap::new(),
                cpu_governor: CpuGovernor::Schedutil,
                io_scheduler: IoScheduler::Auto,
                sched_ext: SchedExt::Off,
            },
            drivers: DriverSpec { gpu: GpuDriver::Auto, gaming_controllers: false },
            packages: PackageSpec {
                system: ["firefox", "git", "btop"].into_iter().map(String::from).collect(),
                flatpak: BTreeSet::new(),
            },
            services: ServiceSpec { enabled: ["flatpak", "fwupd"].into_iter().map(String::from).collect() },
            security: SecuritySpec {
                firewall: FirewallSpec {
                    enabled: true,
                    allowed_tcp_ports: BTreeSet::new(),
                    allowed_udp_ports: BTreeSet::new(),
                },
                mac: MacPolicy::Apparmor,
                usb_storage: UsbStorage::Allow,
                screen_lock: ScreenLockSpec { enabled: true, idle_minutes: 10 },
                audit: true,
            },
            session: SessionSpec { mode: SessionMode::Standard, kiosk_url: None, ephemeral_home: false },
            desktop: DesktopSpec {
                environment: DesktopEnv::Plasma,
                style: StylePreset::GlassDock,
                dock: DockSpec {
                    enabled: true,
                    position: DockPosition::Bottom,
                    autohide: false,
                    size_px: 52,
                    floating: true,
                },
                panel: PanelSpec { position: PanelPosition::Top, global_menu: true, height_px: 28 },
                windows: WindowSpec { snapping: true, tiling: false, gaps_px: 8, corner_radius_px: 12, blur: true },
                workspaces: 4,
                wallpaper: WallpaperSpec { style: WallpaperStyle::Aurora, seed: 7 },
            },
            design: DesignSpec {
                theme_name: "Flux Midnight".into(),
                tokens: DesignTokens {
                    scheme: ColorScheme::Dark,
                    palette,
                    typography: Typography {
                        font_sans: "Inter".into(),
                        font_mono: "JetBrains Mono".into(),
                        font_display: "Inter".into(),
                        base_size_px: 14,
                        scale_ratio: 1.25,
                        line_height: 1.5,
                        weight_regular: 400,
                        weight_bold: 650,
                    },
                    shape: Shape {
                        radius_sm_px: 6,
                        radius_md_px: 10,
                        radius_lg_px: 16,
                        spacing_unit_px: 4,
                        border_width_px: 1,
                    },
                    motion: Motion {
                        duration_fast_ms: 120,
                        duration_base_ms: 220,
                        duration_slow_ms: 360,
                        easing: [0.2, 0.0, 0.0, 1.0],
                        animation_scale: 1.0,
                    },
                    effects: Effects { blur_px: 24, transparency: 0.2, shadow_opacity: 0.35 },
                },
                icon_theme: "Papirus-Dark".into(),
                cursor_theme: "Bibata-Modern-Classic".into(),
            },
            accessibility: AccessibilitySpec {
                high_contrast: false,
                large_text: false,
                text_scale: 1.0,
                screen_reader: false,
                reduce_motion: false,
            },
        }
    }
}

impl SystemSpec {
    /// Stable digest of the spec, used to bind approvals and generations.
    pub fn digest(&self) -> String {
        crate::json_digest(&serde_json::to_value(self).unwrap_or_default())
    }

    pub fn section_value(&self, s: Section) -> serde_json::Value {
        let v = serde_json::to_value(self).unwrap_or_default();
        v.get(section_key(s)).cloned().unwrap_or_default()
    }
}

pub fn section_key(s: Section) -> &'static str {
    match s {
        Section::Base => "base",
        Section::Boot => "boot",
        Section::Kernel => "kernel",
        Section::Drivers => "drivers",
        Section::Packages => "packages",
        Section::Services => "services",
        Section::Security => "security",
        Section::Session => "session",
        Section::Desktop => "desktop",
        Section::Design => "design",
        Section::Accessibility => "accessibility",
    }
}

/// RFC 7386 JSON merge patch, used for profile overlays.
pub fn merge_patch(target: &mut serde_json::Value, patch: &serde_json::Value) {
    use serde_json::Value;
    match patch {
        Value::Object(p) => {
            if !target.is_object() {
                *target = Value::Object(Default::default());
            }
            let t = target.as_object_mut().expect("object");
            for (k, v) in p {
                if v.is_null() {
                    t.remove(k);
                } else {
                    merge_patch(t.entry(k.clone()).or_insert(Value::Null), v);
                }
            }
        }
        other => *target = other.clone(),
    }
}
