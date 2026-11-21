//! Field-level diff between two specs, with a risk tier and a plain-language
//! summary per change. Summaries shown to approvers come from here, derived
//! from the validated state, never from the model's own narrative.

use serde::Serialize;
use serde_json::Value;

use crate::catalog;
use crate::spec::SystemSpec;
use crate::tier::RiskTier;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Change {
    /// Display path, e.g. `kernel.variant` or `kernel.sysctl["vm.swappiness"]`.
    pub path: String,
    #[serde(skip)]
    pub segments: Vec<String>,
    pub before: Value,
    pub after: Value,
    pub tier: RiskTier,
    pub summary: String,
    pub requires_reboot: bool,
}

pub fn diff_specs(a: &SystemSpec, b: &SystemSpec) -> Vec<Change> {
    let va = serde_json::to_value(a).unwrap_or_default();
    let vb = serde_json::to_value(b).unwrap_or_default();
    let mut out = Vec::new();
    walk(&mut Vec::new(), &va, &vb, &mut out);
    out
}

fn walk(path: &mut Vec<String>, a: &Value, b: &Value, out: &mut Vec<Change>) {
    if a == b {
        return;
    }
    match (a, b) {
        (Value::Object(ma), Value::Object(mb)) => {
            let mut keys: Vec<&String> = ma.keys().chain(mb.keys()).collect();
            keys.sort();
            keys.dedup();
            for k in keys {
                path.push(k.clone());
                walk(path, ma.get(k).unwrap_or(&Value::Null), mb.get(k).unwrap_or(&Value::Null), out);
                path.pop();
            }
        }
        _ => out.push(make_change(path.clone(), a.clone(), b.clone())),
    }
}

fn display_path(segments: &[String]) -> String {
    // Sysctl keys contain dots; bracket them so the path stays unambiguous.
    if segments.len() >= 3 && segments[0] == "kernel" && segments[1] == "sysctl" {
        return format!("kernel.sysctl[{:?}]", segments[2..].join("."));
    }
    segments.join(".")
}

fn seg(s: &[String], i: usize) -> &str {
    s.get(i).map(String::as_str).unwrap_or("")
}

/// Risk tier of a single concrete change.
pub fn classify(segments: &[String], before: &Value, after: &Value) -> RiskTier {
    use RiskTier::*;
    match seg(segments, 0) {
        "design" => Safe,
        "accessibility" => {
            if seg(segments, 1) == "screen_reader" {
                Low
            } else {
                Safe
            }
        }
        "desktop" => match seg(segments, 1) {
            "windows" => Low,
            "environment" => Medium,
            _ => Safe,
        },
        "packages" => match seg(segments, 1) {
            "flatpak" => Low,
            _ => Medium,
        },
        "base" => match seg(segments, 1) {
            "hostname" | "timezone" | "locale" => Low,
            _ => High,
        },
        "services" => Medium,
        "security" => match seg(segments, 1) {
            "firewall" if seg(segments, 2) == "enabled" => {
                if before == &Value::Bool(true) && after == &Value::Bool(false) { Blocked } else { Medium }
            }
            "firewall" => Medium,
            "audit" => {
                if before == &Value::Bool(true) && after == &Value::Bool(false) {
                    Blocked
                } else {
                    High
                }
            }
            "screen_lock" => Medium,
            _ => High,
        },
        "kernel" => match seg(segments, 1) {
            "variant" => High,
            "sysctl" if segments[2..].join(".") == "net.ipv4.ip_forward" => High,
            _ => Medium,
        },
        "drivers" | "session" | "boot" => High,
        _ => High,
    }
}

fn requires_reboot(segments: &[String]) -> bool {
    matches!(
        (seg(segments, 0), seg(segments, 1)),
        ("kernel", "variant")
            | ("boot", _)
            | ("drivers", "gpu")
            | ("security", "mac")
            | ("security", "usb_storage")
            | ("session", "ephemeral_home")
    )
}

fn short(v: &Value) -> String {
    match v {
        Value::Null => "unset".into(),
        Value::String(s) => s.clone(),
        Value::Array(items) if items.is_empty() => "none".into(),
        other => other.to_string(),
    }
}

fn set_delta(before: &Value, after: &Value) -> String {
    let as_set = |v: &Value| -> Vec<String> { v.as_array().map(|a| a.iter().map(short).collect()).unwrap_or_default() };
    let (b, a) = (as_set(before), as_set(after));
    let added: Vec<String> = a.iter().filter(|x| !b.contains(x)).map(|x| format!("+{x}")).collect();
    let removed: Vec<String> = b.iter().filter(|x| !a.contains(x)).map(|x| format!("-{x}")).collect();
    let mut parts = added;
    parts.extend(removed);
    parts.join(", ")
}

fn kernel_label(v: &Value) -> String {
    let s = short(v);
    catalog::KERNELS
        .iter()
        .find(|k| serde_json::to_value(k.variant).ok().as_ref() == Some(v))
        .map(|k| format!("{} ({})", k.label, k.version_hint))
        .unwrap_or(s)
}

fn summarize(segments: &[String], before: &Value, after: &Value) -> String {
    let path = segments.join(".");
    let arrow = || format!("{} → {}", short(before), short(after));
    match path.as_str() {
        "kernel.variant" => {
            format!("Kernel: {} → {} (previous boot entry kept)", kernel_label(before), kernel_label(after))
        }
        "kernel.sched_ext" => format!("CPU scheduler (sched_ext): {}, no reboot needed", arrow()),
        "kernel.cpu_governor" => format!("CPU governor: {}", arrow()),
        "kernel.io_scheduler" => format!("I/O scheduler: {}", arrow()),
        "packages.system" => format!("System packages: {}", set_delta(before, after)),
        "packages.flatpak" => format!("User apps (Flathub): {}", set_delta(before, after)),
        "services.enabled" => format!("Services: {}", set_delta(before, after)),
        "security.firewall.enabled" => {
            format!("Firewall: {}", if after == &Value::Bool(false) { "DISABLE (blocked)" } else { "enable" })
        }
        "security.firewall.allowed_tcp_ports" => format!("Inbound TCP ports: {}", set_delta(before, after)),
        "security.firewall.allowed_udp_ports" => format!("Inbound UDP ports: {}", set_delta(before, after)),
        "security.audit" => {
            format!("Audit subsystem: {}", if after == &Value::Bool(false) { "DISABLE (blocked)" } else { "enable" })
        }
        "security.mac" => format!("Mandatory access control: {}", arrow()),
        "security.usb_storage" => format!("USB storage: {}", arrow()),
        "security.screen_lock.idle_minutes" => {
            format!("Screen auto-lock after {} minutes (was {})", short(after), short(before))
        }
        "security.screen_lock.enabled" => format!("Screen auto-lock: {}", arrow()),
        "session.mode" => format!("Session mode: {}", arrow()),
        "session.kiosk_url" => format!("Kiosk URL: {}", arrow()),
        "session.ephemeral_home" => format!("Clean state on every boot (ephemeral home): {}", arrow()),
        "drivers.gpu" => format!("GPU driver: {}", arrow()),
        "drivers.gaming_controllers" => format!("Gaming controller support: {}", arrow()),
        "desktop.environment" => format!("Desktop environment: {}", arrow()),
        "desktop.style" => format!("Style preset: {}", arrow()),
        "design.theme_name" => format!("Theme: {}", arrow()),
        "design.tokens.scheme" => format!("Color scheme: {}", arrow()),
        "design.icon_theme" => format!("Icon theme: {}", arrow()),
        "design.cursor_theme" => format!("Cursor theme: {}", arrow()),
        "boot.splash_theme" => format!("Boot splash theme: {}", arrow()),
        "boot.splash" => format!("Boot splash: {}", arrow()),
        "base.hostname" => format!("Hostname: {}", arrow()),
        "base.timezone" => format!("Time zone: {}", arrow()),
        _ if label(&path).is_some() => {
            let (name, unit) = label(&path).unwrap_or(("", ""));
            let fmt = |v: &Value| match (v, unit) {
                (Value::Bool(true), _) => "on".to_string(),
                (Value::Bool(false), _) => "off".to_string(),
                (Value::Null, _) => "unset".to_string(),
                (other, "") => short(other),
                (other, u) => format!("{} {u}", short(other)),
            };
            format!("{name}: {} → {}", fmt(before), fmt(after))
        }
        _ => {
            if seg(segments, 0) == "kernel" && seg(segments, 1) == "sysctl" {
                let key = segments[2..].join(".");
                let desc = catalog::sysctl(&key).map(|e| format!(" ({})", e.description)).unwrap_or_default();
                return format!("sysctl {key}: {}{desc}", arrow());
            }
            if path.starts_with("design.tokens.palette.") {
                return format!("Color {}: {}", segments.last().map(String::as_str).unwrap_or(""), arrow());
            }
            if path.starts_with("design.tokens.typography.") {
                return format!("Typography {}: {}", segments.last().map(String::as_str).unwrap_or(""), arrow());
            }
            format!("{}: {}", display_path(segments), arrow())
        }
    }
}

/// Human labels (and units) for common leaves.
fn label(path: &str) -> Option<(&'static str, &'static str)> {
    Some(match path {
        "boot.loader_timeout_s" => ("Boot menu timeout", "s"),
        "desktop.dock.enabled" => ("Dock", ""),
        "desktop.dock.position" => ("Dock position", ""),
        "desktop.dock.autohide" => ("Dock auto-hide", ""),
        "desktop.dock.size_px" => ("Dock icon size", "px"),
        "desktop.dock.floating" => ("Floating dock", ""),
        "desktop.panel.position" => ("Panel position", ""),
        "desktop.panel.global_menu" => ("Global menu bar", ""),
        "desktop.panel.height_px" => ("Panel height", "px"),
        "desktop.windows.snapping" => ("Window snapping", ""),
        "desktop.windows.tiling" => ("Tiling window layout", ""),
        "desktop.windows.gaps_px" => ("Window gaps", "px"),
        "desktop.windows.corner_radius_px" => ("Window corner radius", "px"),
        "desktop.windows.blur" => ("Window blur", ""),
        "desktop.workspaces" => ("Workspaces", ""),
        "desktop.wallpaper.style" => ("Wallpaper", ""),
        "desktop.wallpaper.seed" => ("Wallpaper variation", ""),
        "design.tokens.effects.blur_px" => ("Blur radius", "px"),
        "design.tokens.effects.transparency" => ("Translucency", ""),
        "design.tokens.effects.shadow_opacity" => ("Shadow strength", ""),
        "design.tokens.motion.duration_fast_ms" => ("Fast animation", "ms"),
        "design.tokens.motion.duration_base_ms" => ("Standard animation", "ms"),
        "design.tokens.motion.duration_slow_ms" => ("Slow animation", "ms"),
        "design.tokens.motion.easing" => ("Animation curve", ""),
        "design.tokens.motion.animation_scale" => ("Animation scale", "×"),
        "design.tokens.shape.radius_sm_px" => ("Small corner radius", "px"),
        "design.tokens.shape.radius_md_px" => ("Corner radius", "px"),
        "design.tokens.shape.radius_lg_px" => ("Large corner radius", "px"),
        "design.tokens.shape.spacing_unit_px" => ("Spacing unit", "px"),
        "design.tokens.shape.border_width_px" => ("Border width", "px"),
        "design.tokens.typography.base_size_px" => ("Base text size", "px"),
        "design.tokens.typography.scale_ratio" => ("Type scale ratio", ""),
        "design.tokens.typography.line_height" => ("Line height", ""),
        "accessibility.high_contrast" => ("High contrast (WCAG AAA)", ""),
        "accessibility.large_text" => ("Large text", ""),
        "accessibility.text_scale" => ("Text scale", "×"),
        "accessibility.screen_reader" => ("Screen reader", ""),
        "accessibility.reduce_motion" => ("Reduce motion", ""),
        _ => return None,
    })
}

fn make_change(segments: Vec<String>, before: Value, after: Value) -> Change {
    let tier = classify(&segments, &before, &after);
    Change {
        path: display_path(&segments),
        summary: summarize(&segments, &before, &after),
        requires_reboot: requires_reboot(&segments),
        segments,
        before,
        after,
        tier,
    }
}
