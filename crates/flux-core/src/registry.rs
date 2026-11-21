//! Tool registry: the complete allowlist of operations the AI may propose.
//!
//! Exposure is default-deny. A tool that is not listed here cannot be
//! parsed, so it can never reach the applier. Tier-4 tools are listed only so
//! the policy engine can explain why a request is refused.

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Value, json};

use crate::ops::*;
use crate::tier::RiskTier;

pub struct ToolSpec {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    /// Declared tier. The effective tier of a plan is the max of declared
    /// tiers and the tiers computed from the actual state diff.
    pub tier: RiskTier,
    /// Spec section the tool writes to.
    pub scope: &'static str,
    pub blocked_reason: Option<&'static str>,
    pub schema: fn() -> Value,
}

/// Public view of a tool, as served by `GET /v1/tools`.
#[derive(Debug, Clone, Serialize)]
pub struct ToolInfo {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub tier: RiskTier,
    pub tier_label: &'static str,
    pub behavior: &'static str,
    pub scope: &'static str,
    pub blocked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<&'static str>,
    pub params_schema: Value,
}

impl ToolSpec {
    pub fn info(&self) -> ToolInfo {
        ToolInfo {
            name: self.name,
            title: self.title,
            description: self.description,
            tier: self.tier,
            tier_label: self.tier.label(),
            behavior: self.tier.behavior(),
            scope: self.scope,
            blocked: self.blocked_reason.is_some(),
            blocked_reason: self.blocked_reason,
            params_schema: (self.schema)(),
        }
    }
}

/// JSON Schema for `T` with subschemas inlined (friendlier for LLMs).
pub fn schema_of<T: JsonSchema>() -> Value {
    let settings = schemars::generate::SchemaSettings::draft2020_12().with(|s| {
        s.inline_subschemas = true;
    });
    let generator = settings.into_generator();
    let schema = generator.into_root_schema_for::<T>();
    let mut v = serde_json::to_value(schema).unwrap_or_default();
    if let Some(obj) = v.as_object_mut() {
        obj.remove("$schema");
        obj.remove("title");
    }
    v
}

fn no_params() -> Value {
    json!({ "type": "object", "description": "Blocked tool. Never executed." })
}

macro_rules! tool {
    ($name:literal, $title:literal, $tier:ident, $scope:literal, $ty:ty, $desc:literal) => {
        ToolSpec {
            name: $name,
            title: $title,
            description: $desc,
            tier: RiskTier::$tier,
            scope: $scope,
            blocked_reason: None,
            schema: schema_of::<$ty>,
        }
    };
}

macro_rules! blocked {
    ($name:literal, $title:literal, $reason:literal) => {
        ToolSpec {
            name: $name,
            title: $title,
            description: $reason,
            tier: RiskTier::Blocked,
            scope: "none",
            blocked_reason: Some($reason),
            schema: no_params,
        }
    };
}

pub static TOOLS: &[ToolSpec] = &[
    // Tier 0: look and feel
    tool!(
        "design.generate_palette",
        "Generate palette",
        Safe,
        "design",
        GeneratePalette,
        "Derive a complete, contrast-checked color palette from a brand or accent color. Prefer this over set_palette."
    ),
    tool!(
        "design.set_palette",
        "Set palette colors",
        Safe,
        "design",
        SetPalette,
        "Set individual palette colors (#rrggbb) or switch dark/light scheme."
    ),
    tool!(
        "design.set_typography",
        "Set typography",
        Safe,
        "design",
        SetTypography,
        "Fonts (open-licensed catalog only), base size, type scale and line height."
    ),
    tool!("design.set_shape", "Set shape", Safe, "design", SetShape, "Corner radii, spacing unit and border width."),
    tool!(
        "design.set_motion",
        "Set motion",
        Safe,
        "design",
        SetMotion,
        "Animation durations, easing curve and global animation scale. Presets: snappy, smooth, calm, minimal, none."
    ),
    tool!("design.set_effects", "Set effects", Safe, "design", SetEffects, "Blur, translucency and shadow strength."),
    tool!(
        "design.set_icons",
        "Set icon and cursor theme",
        Safe,
        "design",
        SetIcons,
        "Icon and cursor themes from the catalog."
    ),
    tool!(
        "desktop.set_wallpaper",
        "Set wallpaper",
        Safe,
        "desktop",
        SetWallpaper,
        "Original generated wallpaper colored from the current palette: aurora, gradient, mesh, grid or solid."
    ),
    tool!(
        "desktop.set_layout",
        "Set dock and panel layout",
        Safe,
        "desktop",
        SetLayout,
        "Dock (position, autohide, size, floating), top/bottom panel, global menu and workspace count."
    ),
    tool!(
        "accessibility.set",
        "Accessibility",
        Safe,
        "accessibility",
        SetAccessibility,
        "High contrast (WCAG AAA), large text, text scale, screen reader and reduced motion."
    ),
    // Tier 1: user-level behavior and apps
    tool!(
        "desktop.set_windows",
        "Window behavior",
        Low,
        "desktop",
        SetWindows,
        "Window snapping, tiling, gaps, corner radius and blur."
    ),
    tool!(
        "desktop.apply_style",
        "Apply style preset",
        Low,
        "desktop",
        ApplyStyle,
        "Original look-and-behavior presets: glass-dock, snap-layouts, tiling, classic-panel, ops-center, focus."
    ),
    tool!(
        "apps.install",
        "Install user apps",
        Low,
        "packages",
        AppList,
        "Install user-level apps from Flathub (catalog ids)."
    ),
    tool!("apps.remove", "Remove user apps", Low, "packages", AppList, "Remove user-level Flathub apps."),
    tool!(
        "system.set_identity",
        "Hostname, time zone, locale",
        Low,
        "base",
        SetIdentity,
        "Machine hostname, IANA time zone and locale."
    ),
    // Tier 2: system packages, services, network, runtime kernel tuning
    tool!(
        "packages.install",
        "Install system packages",
        Medium,
        "packages",
        PackageList,
        "Install system packages through Nix (catalog ids)."
    ),
    tool!("packages.remove", "Remove system packages", Medium, "packages", PackageList, "Remove system packages."),
    tool!(
        "services.enable",
        "Enable services",
        Medium,
        "services",
        ServiceList,
        "Enable system services from the catalog."
    ),
    tool!("services.disable", "Disable services", Medium, "services", ServiceList, "Disable system services."),
    tool!(
        "firewall.allow",
        "Open firewall ports",
        Medium,
        "security",
        PortSet,
        "Allow inbound TCP/UDP ports. The firewall itself always stays on."
    ),
    tool!(
        "firewall.revoke",
        "Close firewall ports",
        Medium,
        "security",
        PortSet,
        "Close previously opened inbound ports."
    ),
    tool!(
        "kernel.tune",
        "Runtime kernel tuning",
        Medium,
        "kernel",
        KernelTune,
        "Allowlisted sysctl values, CPU frequency governor and I/O scheduler."
    ),
    tool!(
        "kernel.set_scheduler",
        "Swap CPU scheduler",
        Medium,
        "kernel",
        SetScheduler,
        "Load a sched_ext scheduler without a reboot (lavd for gaming, bpfland for desktop, rusty for servers) or turn it off."
    ),
    tool!(
        "desktop.set_environment",
        "Switch desktop environment",
        Medium,
        "desktop",
        SetEnvironment,
        "Plasma, GNOME, Hyprland (tiling) or COSMIC."
    ),
    // Tier 3: kernel, drivers, security policy, session lock-down, boot
    tool!(
        "kernel.select",
        "Switch kernel",
        High,
        "kernel",
        SelectKernel,
        "Pick a tested kernel: lts, latest, low-latency, performance, hardened. Previous boot entry is kept."
    ),
    tool!(
        "drivers.configure",
        "Configure drivers",
        High,
        "drivers",
        ConfigureDrivers,
        "GPU driver (auto, nvidia, nvidia-open) and gaming controller support. Checked against detected hardware."
    ),
    tool!(
        "security.configure",
        "Security policy",
        High,
        "security",
        ConfigureSecurity,
        "AppArmor, USB storage blocking, screen auto-lock and audit."
    ),
    tool!(
        "session.configure",
        "Session lock-down",
        High,
        "session",
        ConfigureSession,
        "Standard, kiosk or exam mode, and ephemeral home (clean state on every boot)."
    ),
    tool!(
        "boot.configure",
        "Boot splash and loader",
        High,
        "boot",
        ConfigureBoot,
        "Boot splash on/off, Plymouth theme, boot menu timeout."
    ),
    tool!(
        "profile.apply",
        "Apply industry profile",
        High,
        "all",
        ApplyProfile,
        "Apply a saved or built-in profile (gaming, education, telecom, finance, healthcare). Tier is computed from the resulting diff."
    ),
    tool!(
        "generation.restore",
        "Restore generation",
        Safe,
        "all",
        RestoreGeneration,
        "Return to a previous system generation (undo). Tier is computed from the resulting diff."
    ),
    // Tier 4: never allowed
    blocked!(
        "shell.exec",
        "Run shell command",
        "The agent never runs arbitrary commands. Every change must be a typed, validated operation."
    ),
    blocked!("disk.wipe", "Wipe disk", "Wiping disks is never allowed by the agent."),
    blocked!("disk.format", "Format disk", "Formatting or repartitioning disks is never allowed by the agent."),
    blocked!(
        "firewall.disable",
        "Disable firewall",
        "Disabling the firewall is never allowed. Open specific ports with firewall.allow instead."
    ),
    blocked!("audit.disable", "Disable audit", "Turning off auditing is never allowed by the agent."),
    blocked!("audit.purge", "Delete audit logs", "The audit log is append-only. Removing entries is never allowed."),
];

pub fn find(name: &str) -> Option<&'static ToolSpec> {
    TOOLS.iter().find(|t| t.name == name)
}

pub fn infos() -> Vec<ToolInfo> {
    TOOLS.iter().map(ToolSpec::info).collect()
}

/// Parses tool parameters into a typed operation. Unknown fields are errors.
pub fn parse_op(name: &str, params: Value) -> Result<Op, String> {
    use serde_json::from_value as p;
    let params = if params.is_null() { json!({}) } else { params };
    let e = |err: serde_json::Error| err.to_string();
    Ok(match name {
        "design.generate_palette" => Op::GeneratePalette(p(params).map_err(e)?),
        "design.set_palette" => Op::SetPalette(p(params).map_err(e)?),
        "design.set_typography" => Op::SetTypography(p(params).map_err(e)?),
        "design.set_shape" => Op::SetShape(p(params).map_err(e)?),
        "design.set_motion" => Op::SetMotion(p(params).map_err(e)?),
        "design.set_effects" => Op::SetEffects(p(params).map_err(e)?),
        "design.set_icons" => Op::SetIcons(p(params).map_err(e)?),
        "desktop.set_wallpaper" => Op::SetWallpaper(p(params).map_err(e)?),
        "desktop.set_layout" => Op::SetLayout(p(params).map_err(e)?),
        "accessibility.set" => Op::SetAccessibility(p(params).map_err(e)?),
        "desktop.set_windows" => Op::SetWindows(p(params).map_err(e)?),
        "desktop.apply_style" => Op::ApplyStyle(p(params).map_err(e)?),
        "apps.install" => Op::AppsInstall(p(params).map_err(e)?),
        "apps.remove" => Op::AppsRemove(p(params).map_err(e)?),
        "system.set_identity" => Op::SetIdentity(p(params).map_err(e)?),
        "packages.install" => Op::PackagesInstall(p(params).map_err(e)?),
        "packages.remove" => Op::PackagesRemove(p(params).map_err(e)?),
        "services.enable" => Op::ServicesEnable(p(params).map_err(e)?),
        "services.disable" => Op::ServicesDisable(p(params).map_err(e)?),
        "firewall.allow" => Op::FirewallAllow(p(params).map_err(e)?),
        "firewall.revoke" => Op::FirewallRevoke(p(params).map_err(e)?),
        "kernel.tune" => Op::KernelTune(p(params).map_err(e)?),
        "kernel.set_scheduler" => Op::SetScheduler(p(params).map_err(e)?),
        "desktop.set_environment" => Op::SetEnvironment(p(params).map_err(e)?),
        "kernel.select" => Op::SelectKernel(p(params).map_err(e)?),
        "drivers.configure" => Op::ConfigureDrivers(p(params).map_err(e)?),
        "security.configure" => Op::ConfigureSecurity(p(params).map_err(e)?),
        "session.configure" => Op::ConfigureSession(p(params).map_err(e)?),
        "boot.configure" => Op::ConfigureBoot(p(params).map_err(e)?),
        "profile.apply" => Op::ApplyProfile(p(params).map_err(e)?),
        "generation.restore" => Op::RestoreGeneration(p(params).map_err(e)?),
        other => return Err(format!("tool {other:?} has no parser")),
    })
}
