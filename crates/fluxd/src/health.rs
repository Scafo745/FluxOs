//! Health checks. Render-level checks run for every preview; targets add
//! their own system checks after an apply. Any failure triggers rollback.

use flux_core::render::Artifact;
use flux_core::{SystemSpec, catalog, color, nixsyntax, tokens};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResult {
    pub id: String,
    pub name: String,
    pub status: HealthStatus,
    pub detail: String,
}

impl HealthResult {
    pub fn pass(id: &str, name: &str, detail: impl Into<String>) -> Self {
        Self { id: id.into(), name: name.into(), status: HealthStatus::Pass, detail: detail.into() }
    }
    pub fn warn(id: &str, name: &str, detail: impl Into<String>) -> Self {
        Self { id: id.into(), name: name.into(), status: HealthStatus::Warn, detail: detail.into() }
    }
    pub fn fail(id: &str, name: &str, detail: impl Into<String>) -> Self {
        Self { id: id.into(), name: name.into(), status: HealthStatus::Fail, detail: detail.into() }
    }
}

pub fn any_failed(results: &[HealthResult]) -> bool {
    results.iter().any(|r| r.status == HealthStatus::Fail)
}

pub const REQUIRED: &[&str] = &[
    "etc/nixos/fluxos/generated.nix",
    "etc/fluxos/spec.json",
    "etc/fluxos/tokens.json",
    "etc/fluxos/theme.css",
    "etc/fluxos/wallpaper.svg",
];

fn find<'a>(artifacts: &'a [Artifact], path: &str) -> Option<&'a Artifact> {
    artifacts.iter().find(|a| a.path == path)
}

/// Checks that need only the rendered artifacts (no running system).
pub fn artifact_checks(spec: &SystemSpec, artifacts: &[Artifact]) -> Vec<HealthResult> {
    let mut out = Vec::new();

    let missing: Vec<&str> = REQUIRED.iter().copied().filter(|p| find(artifacts, p).is_none()).collect();
    out.push(if missing.is_empty() {
        HealthResult::pass("artifacts.complete", "Artifacts complete", format!("{} files rendered", artifacts.len()))
    } else {
        HealthResult::fail("artifacts.complete", "Artifacts complete", format!("missing: {}", missing.join(", ")))
    });

    match find(artifacts, "etc/nixos/fluxos/generated.nix") {
        Some(a) => out.push(match nixsyntax::check(&a.content) {
            Ok(()) => HealthResult::pass(
                "nix.syntax",
                "NixOS module syntax",
                format!("{} lines, balanced", a.content.lines().count()),
            ),
            Err(e) => {
                HealthResult::fail("nix.syntax", "NixOS module syntax", format!("line {}: {}", e.line, e.message))
            }
        }),
        None => out.push(HealthResult::fail("nix.syntax", "NixOS module syntax", "generated.nix missing")),
    }

    match find(artifacts, "etc/fluxos/tokens.json").map(|a| serde_json::from_str::<serde_json::Value>(&a.content)) {
        Some(Ok(doc)) => out.push(match tokens::check_dtcg(&doc) {
            Ok(n) => HealthResult::pass("tokens.dtcg", "Design tokens (DTCG 2025.10)", format!("{n} typed tokens")),
            Err(e) => HealthResult::fail("tokens.dtcg", "Design tokens (DTCG 2025.10)", e),
        }),
        Some(Err(e)) => {
            out.push(HealthResult::fail("tokens.dtcg", "Design tokens (DTCG 2025.10)", format!("invalid JSON: {e}")))
        }
        None => out.push(HealthResult::fail("tokens.dtcg", "Design tokens (DTCG 2025.10)", "tokens.json missing")),
    }

    let p = &spec.design.tokens.palette;
    let min = if spec.accessibility.high_contrast { 7.0 } else { 4.5 };
    out.push(match color::contrast_hex(&p.text, &p.background) {
        Some(r) if r >= min => {
            HealthResult::pass("design.contrast", "Text contrast", format!("{r:.2}:1 (needs {min}:1)"))
        }
        Some(r) => HealthResult::fail("design.contrast", "Text contrast", format!("{r:.2}:1 is below {min}:1")),
        None => HealthResult::fail("design.contrast", "Text contrast", "palette has invalid colors"),
    });

    if let Some(a) = find(artifacts, "etc/sysctl.d/90-fluxos.conf") {
        let bad: Vec<String> = a
            .content
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .filter(|l| match l.split_once(" = ") {
                Some((k, _)) => catalog::sysctl(k.trim()).is_none(),
                None => true,
            })
            .map(String::from)
            .collect();
        out.push(if bad.is_empty() {
            HealthResult::pass("kernel.sysctl", "Kernel parameters", "all keys allowlisted")
        } else {
            HealthResult::fail("kernel.sysctl", "Kernel parameters", format!("unexpected lines: {}", bad.join("; ")))
        });
    }

    if let Some(a) = find(artifacts, "etc/fluxos/wallpaper.svg") {
        let ok = a.content.starts_with("<svg") && a.content.trim_end().ends_with("</svg>");
        out.push(if ok {
            HealthResult::pass("design.wallpaper", "Wallpaper", format!("{} bytes of original SVG", a.bytes))
        } else {
            HealthResult::fail("design.wallpaper", "Wallpaper", "SVG is malformed")
        });
    }
    out
}
