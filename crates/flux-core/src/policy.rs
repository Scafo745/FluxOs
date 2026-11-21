//! The policy engine: the trust boundary between model output and the system.
//!
//! [`evaluate`] takes untrusted operations, rejects anything outside the
//! registry, simulates the rest on a copy of the current spec, validates the
//! candidate (schema ranges, catalogs, contrast, hardware compatibility) and
//! computes the effective risk tier from the actual diff.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::catalog;
use crate::color;
use crate::diff::{Change, diff_specs};
use crate::hardware::{GpuVendor, HardwareInfo};
use crate::ops::OpContext;
use crate::profiles::ProfileSet;
use crate::registry;
use crate::spec::*;
use crate::tier::RiskTier;

pub const MAX_OPERATIONS: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Issue {
    pub severity: Severity,
    pub code: String,
    pub path: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub op: Option<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub suggestions: Vec<String>,
}

impl Issue {
    pub fn error(code: &str, path: &str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code: code.into(),
            path: path.into(),
            message: message.into(),
            op: None,
            suggestions: vec![],
        }
    }

    pub fn warning(code: &str, path: &str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            code: code.into(),
            path: path.into(),
            message: message.into(),
            op: None,
            suggestions: vec![],
        }
    }

    pub fn with_op(mut self, op: usize) -> Self {
        self.op = Some(op);
        self
    }

    pub fn suggest(mut self, s: Vec<String>) -> Self {
        self.suggestions = s;
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

/// An operation exactly as proposed by the model or a client (untrusted).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawOp {
    pub tool: String,
    #[serde(default)]
    pub params: Value,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpReport {
    pub index: usize,
    pub tool: String,
    pub declared_tier: Option<RiskTier>,
    pub ok: bool,
}

pub struct EvalContext<'a> {
    pub profiles: &'a ProfileSet,
    pub hardware: &'a HardwareInfo,
    pub generation: &'a dyn Fn(u64) -> Option<SystemSpec>,
    /// Tools the operator switched off in `policy.json`.
    pub disabled_tools: &'a [String],
}

#[derive(Debug, Clone, Serialize)]
pub struct Evaluation {
    /// No errors and the tier is not blocked.
    pub ok: bool,
    pub blocked: bool,
    pub tier: RiskTier,
    pub operations: Vec<OpReport>,
    pub issues: Vec<Issue>,
    pub changes: Vec<Change>,
    pub requires_reboot: bool,
    #[serde(skip)]
    pub candidate: Option<SystemSpec>,
}

impl Evaluation {
    pub fn errors(&self) -> impl Iterator<Item = &Issue> {
        self.issues.iter().filter(|i| i.is_error())
    }

    pub fn no_changes(&self) -> bool {
        self.ok && self.changes.is_empty()
    }
}

/// Simulates and validates `ops` against `current`.
pub fn evaluate(current: &SystemSpec, ops: &[RawOp], ctx: &EvalContext<'_>) -> Evaluation {
    let mut issues = Vec::new();
    let mut reports = Vec::new();
    let mut declared = RiskTier::Safe;
    let mut blocked = false;

    if ops.is_empty() {
        issues.push(Issue::error("empty_plan", "operations", "the plan has no operations"));
    }
    if ops.len() > MAX_OPERATIONS {
        issues.push(Issue::error(
            "too_many_operations",
            "operations",
            format!("at most {MAX_OPERATIONS} operations per plan"),
        ));
    }

    let op_ctx = OpContext { profiles: ctx.profiles, generation: ctx.generation };
    let mut candidate = current.clone();

    for (i, raw) in ops.iter().enumerate().take(MAX_OPERATIONS) {
        let path = format!("operations[{i}]");
        let Some(tool) = registry::find(&raw.tool) else {
            let names = registry::TOOLS.iter().filter(|t| t.blocked_reason.is_none()).map(|t| t.name);
            issues.push(
                Issue::error(
                    "unknown_tool",
                    &path,
                    format!("tool {:?} is not in the registry (default-deny)", raw.tool),
                )
                .with_op(i)
                .suggest(catalog::suggest(&raw.tool, names, 3)),
            );
            reports.push(OpReport { index: i, tool: raw.tool.clone(), declared_tier: None, ok: false });
            continue;
        };
        if let Some(reason) = tool.blocked_reason {
            blocked = true;
            issues.push(Issue::error("blocked_tool", &path, reason).with_op(i));
            reports.push(OpReport {
                index: i,
                tool: raw.tool.clone(),
                declared_tier: Some(RiskTier::Blocked),
                ok: false,
            });
            continue;
        }
        if ctx.disabled_tools.iter().any(|t| t == tool.name) {
            issues.push(
                Issue::error("disabled_tool", &path, format!("{} is disabled by operator policy", tool.name))
                    .with_op(i),
            );
            reports.push(OpReport { index: i, tool: raw.tool.clone(), declared_tier: Some(tool.tier), ok: false });
            continue;
        }
        declared = declared.max(tool.tier);
        let op = match registry::parse_op(tool.name, raw.params.clone()) {
            Ok(op) => op,
            Err(e) => {
                issues.push(
                    Issue::error("invalid_params", &format!("{path}.params"), format!("{}: {e}", tool.name)).with_op(i),
                );
                reports.push(OpReport { index: i, tool: raw.tool.clone(), declared_tier: Some(tool.tier), ok: false });
                continue;
            }
        };
        let op_issues = op.apply(&mut candidate, &op_ctx);
        let ok = !op_issues.iter().any(Issue::is_error);
        issues.extend(op_issues.into_iter().map(|mut is| {
            is.path = format!("{path}.{}", is.path);
            is.with_op(i)
        }));
        reports.push(OpReport { index: i, tool: raw.tool.clone(), declared_tier: Some(tool.tier), ok });
    }

    let op_errors = issues.iter().any(Issue::is_error);
    issues.extend(validate_spec(&candidate, ctx.hardware));

    let mut changes = diff_specs(current, &candidate);
    // Approvers read the riskiest changes first.
    changes.sort_by(|a, b| b.tier.cmp(&a.tier).then_with(|| a.path.cmp(&b.path)));
    let diff_tier = changes.iter().map(|c| c.tier).max().unwrap_or(RiskTier::Safe);
    if diff_tier == RiskTier::Blocked {
        blocked = true;
        for c in changes.iter().filter(|c| c.tier == RiskTier::Blocked) {
            issues.push(Issue::error(
                "blocked_change",
                &c.path,
                format!("{} is never allowed by the agent", c.summary),
            ));
        }
    }
    let tier = if blocked { RiskTier::Blocked } else { declared.max(diff_tier) };
    if !op_errors && changes.is_empty() && !ops.is_empty() {
        issues.push(Issue::warning("no_changes", "operations", "the plan does not change the current system"));
    }
    let requires_reboot = changes.iter().any(|c| c.requires_reboot);
    let ok = !issues.iter().any(Issue::is_error) && !blocked;

    Evaluation {
        ok,
        blocked,
        tier,
        operations: reports,
        issues,
        changes,
        requires_reboot,
        candidate: if ok { Some(candidate) } else { None },
    }
}

fn in_range<T: PartialOrd + std::fmt::Display + Copy>(issues: &mut Vec<Issue>, path: &str, v: T, lo: T, hi: T) {
    if v < lo || v > hi {
        issues.push(Issue::error("out_of_range", path, format!("{path} must be between {lo} and {hi}, got {v}")));
    }
}

fn valid_hostname(h: &str) -> bool {
    !h.is_empty()
        && h.len() <= 63
        && h.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !h.starts_with('-')
        && !h.ends_with('-')
}

fn valid_timezone(tz: &str) -> bool {
    if tz == "UTC" {
        return true;
    }
    let parts: Vec<&str> = tz.split('/').collect();
    (2..=3).contains(&parts.len())
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '+'))
        && parts[0].chars().next().is_some_and(|c| c.is_ascii_uppercase())
}

fn valid_locale(l: &str) -> bool {
    let Some((lang, enc)) = l.split_once('.') else { return false };
    let Some((a, b)) = lang.split_once('_') else { return false };
    (2..=3).contains(&a.len())
        && a.chars().all(|c| c.is_ascii_lowercase())
        && b.len() == 2
        && b.chars().all(|c| c.is_ascii_uppercase())
        && enc == "UTF-8"
}

/// Strict URL check for kiosk mode: https only, no characters that could be
/// interpreted by systemd, a shell or Nix.
pub fn valid_kiosk_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    if url.len() > 512 || rest.is_empty() {
        return false;
    }
    let (host, tail) = rest.split_at(rest.find(['/', '?']).unwrap_or(rest.len()));
    let host_ok = !host.is_empty()
        && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == ':')
        && !host.starts_with('.');
    let tail_ok = tail.chars().all(|c| c.is_ascii_alphanumeric() || "/._~-%?=&+#".contains(c));
    host_ok && tail_ok
}

/// Validates a whole spec. Used for every candidate and for stored profiles.
pub fn validate_spec(spec: &SystemSpec, hw: &HardwareInfo) -> Vec<Issue> {
    let mut v = Vec::new();

    // Base
    if spec.base.distro != "nixos" {
        v.push(Issue::error("unsupported_distro", "base.distro", "only the nixos base is supported in v1"));
    }
    if !["26.05", "26.11"].contains(&spec.base.release.as_str()) {
        v.push(Issue::error(
            "unsupported_release",
            "base.release",
            format!("release {:?} is not supported (26.05, 26.11)", spec.base.release),
        ));
    }
    if !valid_hostname(&spec.base.hostname) {
        v.push(Issue::error("invalid_hostname", "base.hostname", "hostname must be 1-63 chars of a-z, 0-9 and '-'"));
    }
    if !valid_timezone(&spec.base.timezone) {
        v.push(Issue::error(
            "invalid_timezone",
            "base.timezone",
            format!("{:?} is not an IANA time zone like Europe/Berlin", spec.base.timezone),
        ));
    }
    if !valid_locale(&spec.base.locale) {
        v.push(Issue::error(
            "invalid_locale",
            "base.locale",
            format!("{:?} is not a locale like en_US.UTF-8", spec.base.locale),
        ));
    }

    // Boot
    if !catalog::splash_known(&spec.boot.splash_theme) {
        v.push(
            Issue::error(
                "unknown_splash_theme",
                "boot.splash_theme",
                format!("{:?} is not in the boot splash catalog", spec.boot.splash_theme),
            )
            .suggest(catalog::suggest(
                &spec.boot.splash_theme,
                catalog::SPLASH_BUILTIN.iter().chain(catalog::SPLASH_ADI1090X).copied(),
                3,
            )),
        );
    }
    in_range(&mut v, "boot.loader_timeout_s", spec.boot.loader_timeout_s, 0, 30);

    // Kernel
    for (key, value) in &spec.kernel.sysctl {
        if let Err(msg) = catalog::check_sysctl(key, value) {
            let s = if catalog::sysctl(key).is_none() {
                catalog::suggest(key, catalog::SYSCTLS.iter().map(|e| e.key), 3)
            } else {
                vec![]
            };
            v.push(Issue::error("invalid_sysctl", &format!("kernel.sysctl[{key:?}]"), msg).suggest(s));
        }
    }
    let sched = catalog::scheduler(spec.kernel.sched_ext);
    if sched.maturity == "experimental" {
        v.push(Issue::warning(
            "experimental_scheduler",
            "kernel.sched_ext",
            format!(
                "{} is experimental; validate it on this workload before relying on it",
                sched.binary.unwrap_or("scheduler")
            ),
        ));
    }
    if spec.kernel.sched_ext != SchedExt::Off && hw.gpus_known() && hw.is_nixos && !hw.sched_ext_supported {
        v.push(Issue::warning(
            "sched_ext_unavailable",
            "kernel.sched_ext",
            "the running kernel does not expose sched_ext; it activates after booting a 6.12+ kernel",
        ));
    }
    if let Some(drv) = &hw.cpu.freq_driver
        && (drv.contains("pstate"))
        && !matches!(spec.kernel.cpu_governor, CpuGovernor::Performance | CpuGovernor::Powersave)
    {
        v.push(Issue::warning(
            "governor_unavailable",
            "kernel.cpu_governor",
            format!("{drv} only offers performance and powersave governors"),
        ));
    }

    // Packages and services
    for id in &spec.packages.system {
        match catalog::package(id) {
            Some(p) if matches!(p.nix, catalog::NixInstall::FlatpakOnly) => {
                v.push(Issue::error("flatpak_only", "packages.system", format!("{id} is only available from Flathub")))
            }
            Some(_) => {}
            None => v.push(Issue::error(
                "unknown_package",
                "packages.system",
                format!("{id:?} is not in the package catalog"),
            )),
        }
    }
    for id in &spec.packages.flatpak {
        match catalog::package(id) {
            Some(p) if p.flatpak.is_some() => {}
            _ => v.push(Issue::error(
                "unknown_app",
                "packages.flatpak",
                format!("{id:?} has no Flathub build in the catalog"),
            )),
        }
    }
    if !spec.packages.flatpak.is_empty() && !spec.services.enabled.contains("flatpak") {
        v.push(Issue::error(
            "flatpak_disabled",
            "services.enabled",
            "user apps need the flatpak service; enable it with services.enable [\"flatpak\"]",
        ));
    }
    for id in &spec.services.enabled {
        match catalog::service(id) {
            Some(s) => {
                for port in s.opens_tcp {
                    v.push(Issue::warning(
                        "opens_port",
                        "services.enabled",
                        format!("{} opens inbound TCP port {port}", s.name),
                    ));
                }
            }
            None => v.push(Issue::error(
                "unknown_service",
                "services.enabled",
                format!("{id:?} is not in the service catalog"),
            )),
        }
    }

    // Security
    if !spec.security.firewall.enabled {
        v.push(Issue::error("firewall_disabled", "security.firewall.enabled", "the firewall must stay enabled"));
    }
    if !spec.security.audit {
        v.push(Issue::error("audit_disabled", "security.audit", "auditing must stay enabled"));
    }
    for p in spec.security.firewall.allowed_tcp_ports.iter().chain(&spec.security.firewall.allowed_udp_ports) {
        if *p == 0 {
            v.push(Issue::error("invalid_port", "security.firewall", "port 0 is not valid"));
        }
    }
    in_range(&mut v, "security.screen_lock.idle_minutes", spec.security.screen_lock.idle_minutes, 1, 240);
    if spec.security.mac == MacPolicy::Off {
        v.push(Issue::warning(
            "mac_off",
            "security.mac",
            "AppArmor is off; applications run without mandatory access control",
        ));
    }

    // Session
    match spec.session.mode {
        SessionMode::Kiosk | SessionMode::Exam => match &spec.session.kiosk_url {
            Some(url) if valid_kiosk_url(url) => {}
            Some(url) => v.push(Issue::error(
                "invalid_kiosk_url",
                "session.kiosk_url",
                format!("{url:?} must be a plain https:// URL"),
            )),
            None => {
                v.push(Issue::error("missing_kiosk_url", "session.kiosk_url", "kiosk and exam modes need a kiosk_url"))
            }
        },
        SessionMode::Standard => {}
    }
    if spec.session.mode == SessionMode::Exam && spec.security.usb_storage != UsbStorage::Block {
        v.push(Issue::warning("exam_usb", "security.usb_storage", "exam mode usually blocks USB storage"));
    }
    if spec.session.ephemeral_home {
        v.push(Issue::warning(
            "ephemeral_home",
            "session.ephemeral_home",
            "files in the shared account's home are erased on every boot",
        ));
    }

    // Drivers and hardware compatibility
    if matches!(spec.drivers.gpu, GpuDriver::Nvidia | GpuDriver::NvidiaOpen) {
        if hw.gpus_known() && !hw.has_gpu(GpuVendor::Nvidia) {
            v.push(Issue::error("hardware_mismatch", "drivers.gpu", "no NVIDIA GPU was detected on this machine"));
        }
        if hw.secure_boot == Some(true) {
            v.push(Issue::warning(
                "secure_boot",
                "drivers.gpu",
                "Secure Boot is on: NVIDIA kernel modules must be signed or they will not load",
            ));
        }
    }
    if spec.kernel.variant != KernelVariant::Lts
        && spec.kernel.variant != KernelVariant::Hardened
        && hw.secure_boot == Some(true)
    {
        v.push(Issue::warning(
            "secure_boot",
            "kernel.variant",
            "Secure Boot is on: confirm this kernel is signed for your boot chain",
        ));
    }

    // Desktop
    let d = &spec.desktop;
    in_range(&mut v, "desktop.dock.size_px", d.dock.size_px, 24, 96);
    in_range(&mut v, "desktop.panel.height_px", d.panel.height_px, 20, 64);
    in_range(&mut v, "desktop.windows.gaps_px", d.windows.gaps_px, 0, 32);
    in_range(&mut v, "desktop.windows.corner_radius_px", d.windows.corner_radius_px, 0, 24);
    in_range(&mut v, "desktop.workspaces", d.workspaces, 1, 12);
    if d.environment == DesktopEnv::Cosmic {
        v.push(Issue::warning(
            "cosmic_theming",
            "desktop.environment",
            "COSMIC theme export is not implemented yet; tokens apply to GTK and web apps only",
        ));
    }

    // Design
    let t = &spec.design.tokens;
    if spec.design.theme_name.trim().is_empty() || spec.design.theme_name.len() > 48 {
        v.push(Issue::error("invalid_theme_name", "design.theme_name", "theme name must be 1-48 characters"));
    }
    if spec.design.theme_name.chars().any(|c| c.is_control() || "\"'\\$`{}".contains(c)) {
        v.push(Issue::error("invalid_theme_name", "design.theme_name", "theme name contains forbidden characters"));
    }
    for (name, hex) in t.palette.entries() {
        if !color::is_hex6(hex) {
            v.push(Issue::error(
                "invalid_color",
                &format!("design.tokens.palette.{name}"),
                format!("{hex:?} must be #rrggbb"),
            ));
        }
    }
    let p = &t.palette;
    if let (Some(text_c), Some(muted_c), Some(accent_c)) = (
        color::contrast_hex(&p.text, &p.background),
        color::contrast_hex(&p.text_muted, &p.background),
        color::contrast_hex(&p.accent_text, &p.accent),
    ) {
        let min = if spec.accessibility.high_contrast { 7.0 } else { 4.5 };
        if text_c < min {
            v.push(Issue::error(
                "low_contrast",
                "design.tokens.palette.text",
                format!("text on background is {text_c:.2}:1; needs {min}:1 (use design.generate_palette to fix)"),
            ));
        }
        if muted_c < 3.0 {
            v.push(Issue::error(
                "low_contrast",
                "design.tokens.palette.text_muted",
                format!("muted text on background is {muted_c:.2}:1; needs 3:1"),
            ));
        } else if muted_c < min {
            v.push(Issue::warning(
                "low_contrast",
                "design.tokens.palette.text_muted",
                format!("muted text on background is {muted_c:.2}:1 (target {min}:1)"),
            ));
        }
        if accent_c < 4.5 {
            v.push(Issue::warning(
                "low_contrast",
                "design.tokens.palette.accent_text",
                format!("text on accent is {accent_c:.2}:1 (target 4.5:1)"),
            ));
        }
    }
    for (path, fam, kind) in [
        ("design.tokens.typography.font_sans", &t.typography.font_sans, "sans"),
        ("design.tokens.typography.font_mono", &t.typography.font_mono, "mono"),
        ("design.tokens.typography.font_display", &t.typography.font_display, "display"),
    ] {
        match catalog::font(fam) {
            Some(f) if kind == "mono" && f.kind != "mono" => {
                v.push(Issue::warning("font_kind", path, format!("{fam} is not a monospace font")))
            }
            Some(_) => {}
            None => v.push(
                Issue::error("unknown_font", path, format!("{fam:?} is not in the open-licensed font catalog"))
                    .suggest(catalog::suggest(fam, catalog::FONTS.iter().map(|f| f.family), 3)),
            ),
        }
    }
    in_range(&mut v, "design.tokens.typography.base_size_px", t.typography.base_size_px, 11, 24);
    in_range(&mut v, "design.tokens.typography.scale_ratio", t.typography.scale_ratio, 1.05, 1.5);
    in_range(&mut v, "design.tokens.typography.line_height", t.typography.line_height, 1.1, 2.0);
    in_range(&mut v, "design.tokens.typography.weight_regular", t.typography.weight_regular, 100, 600);
    in_range(&mut v, "design.tokens.typography.weight_bold", t.typography.weight_bold, 500, 1000);
    for (path, r) in [
        ("design.tokens.shape.radius_sm_px", t.shape.radius_sm_px),
        ("design.tokens.shape.radius_md_px", t.shape.radius_md_px),
        ("design.tokens.shape.radius_lg_px", t.shape.radius_lg_px),
    ] {
        in_range(&mut v, path, r, 0, 32);
    }
    in_range(&mut v, "design.tokens.shape.spacing_unit_px", t.shape.spacing_unit_px, 2, 12);
    in_range(&mut v, "design.tokens.shape.border_width_px", t.shape.border_width_px, 0, 4);
    for (path, ms) in [
        ("design.tokens.motion.duration_fast_ms", t.motion.duration_fast_ms),
        ("design.tokens.motion.duration_base_ms", t.motion.duration_base_ms),
        ("design.tokens.motion.duration_slow_ms", t.motion.duration_slow_ms),
    ] {
        in_range(&mut v, path, ms, 0, 2000);
    }
    let [x1, y1, x2, y2] = t.motion.easing;
    if !(0.0..=1.0).contains(&x1)
        || !(0.0..=1.0).contains(&x2)
        || !y1.is_finite()
        || !y2.is_finite()
        || y1.abs() > 3.0
        || y2.abs() > 3.0
    {
        v.push(Issue::error(
            "invalid_easing",
            "design.tokens.motion.easing",
            "cubic-bezier x values must be in 0..1 and y values in -3..3",
        ));
    }
    in_range(&mut v, "design.tokens.motion.animation_scale", t.motion.animation_scale, 0.0, 3.0);
    in_range(&mut v, "design.tokens.effects.blur_px", t.effects.blur_px, 0, 64);
    in_range(&mut v, "design.tokens.effects.transparency", t.effects.transparency, 0.0, 0.6);
    in_range(&mut v, "design.tokens.effects.shadow_opacity", t.effects.shadow_opacity, 0.0, 0.8);
    if catalog::icon_theme(&spec.design.icon_theme).is_none() {
        v.push(
            Issue::error(
                "unknown_icon_theme",
                "design.icon_theme",
                format!("{:?} is not in the icon catalog", spec.design.icon_theme),
            )
            .suggest(catalog::suggest(
                &spec.design.icon_theme,
                catalog::ICON_THEMES.iter().map(|t| t.name),
                3,
            )),
        );
    }
    if catalog::cursor_theme(&spec.design.cursor_theme).is_none() {
        v.push(
            Issue::error(
                "unknown_cursor_theme",
                "design.cursor_theme",
                format!("{:?} is not in the cursor catalog", spec.design.cursor_theme),
            )
            .suggest(catalog::suggest(
                &spec.design.cursor_theme,
                catalog::CURSOR_THEMES.iter().map(|t| t.name),
                3,
            )),
        );
    }

    // Accessibility
    in_range(&mut v, "accessibility.text_scale", spec.accessibility.text_scale, 1.0, 2.0);

    v
}

// ------------------------------------------------------------- decisions --

/// Deployment mode. In fleets, tier 2+ changes need an administrator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    Personal,
    Fleet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// Tier 0-1: applied immediately with a snapshot; undo offered.
    AutoApply,
    /// Tier 2-3 on a personal device: the owner confirms.
    Confirm,
    /// Tier 2-3 in a fleet: an administrator approves.
    AdminApproval,
    /// Tier 4 or invalid.
    Blocked,
}

pub fn decide(tier: RiskTier, mode: Mode) -> Decision {
    match (tier, mode) {
        (RiskTier::Blocked, _) => Decision::Blocked,
        (RiskTier::Safe | RiskTier::Low, _) => Decision::AutoApply,
        (_, Mode::Personal) => Decision::Confirm,
        (_, Mode::Fleet) => Decision::AdminApproval,
    }
}

/// A restricted-change window, e.g. no tier 2+ changes during trading hours.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreezeWindow {
    pub name: String,
    /// ISO weekdays, 1 = Monday ... 7 = Sunday.
    pub days: Vec<u8>,
    /// Local start time "HH:MM".
    pub start: String,
    /// Local end time "HH:MM".
    pub end: String,
    /// Offset of the local time zone from UTC in minutes (e.g. -300 for New York, standard time).
    pub utc_offset_minutes: i32,
    /// Changes at or above this tier are deferred during the window.
    pub min_tier: u8,
}

fn hhmm(s: &str) -> Option<u32> {
    let (h, m) = s.split_once(':')?;
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

impl FreezeWindow {
    /// Whether the window is active at `unix_secs` for a change of `tier`.
    pub fn blocks(&self, unix_secs: i64, tier: RiskTier) -> bool {
        if tier.as_u8() < self.min_tier || tier == RiskTier::Blocked {
            return false;
        }
        let (Some(start), Some(end)) = (hhmm(&self.start), hhmm(&self.end)) else { return false };
        let local = unix_secs + self.utc_offset_minutes as i64 * 60;
        let days = local.div_euclid(86_400);
        // 1970-01-01 was a Thursday (ISO 4).
        let weekday = ((days + 3).rem_euclid(7) + 1) as u8;
        let minute = (local.rem_euclid(86_400) / 60) as u32;
        self.days.contains(&weekday) && minute >= start && minute < end
    }

    pub fn validate(&self) -> Result<(), String> {
        if hhmm(&self.start).is_none() || hhmm(&self.end).is_none() {
            return Err(format!("freeze window {:?}: times must be HH:MM", self.name));
        }
        if self.days.iter().any(|d| !(1..=7).contains(d)) {
            return Err(format!("freeze window {:?}: days are 1 (Mon) to 7 (Sun)", self.name));
        }
        Ok(())
    }
}

/// Operator policy loaded by the daemon (`policy.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    #[serde(default = "default_mode")]
    pub mode: Mode,
    #[serde(default)]
    pub freeze_windows: Vec<FreezeWindow>,
    /// Tools disabled by the operator on top of the registry (still default-deny for unknown tools).
    #[serde(default)]
    pub disabled_tools: Vec<String>,
}

fn default_mode() -> Mode {
    Mode::Personal
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self { mode: Mode::Personal, freeze_windows: vec![], disabled_tools: vec![] }
    }
}
