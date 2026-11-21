//! Apply targets: the adapter layer between generations and a real system.
//!
//! * [`sandbox::SandboxTarget`] materializes the system into the state
//!   directory. It is the default and safe on any Linux host.
//! * [`nixos::NixosTarget`] writes the generated NixOS module and drives
//!   `nixos-rebuild`, keeping previous boot entries for rollback.

pub mod nixos;
pub mod sandbox;

use std::path::{Path, PathBuf};

use anyhow::Result;
use flux_core::SystemSpec;
use flux_core::render::Artifact;
use serde_json::Value;

use crate::health::HealthResult;
use crate::store::ArtifactInfo;

pub struct ApplyRequest<'a> {
    pub generation: u64,
    pub previous: u64,
    pub spec: &'a SystemSpec,
    pub artifacts: &'a [Artifact],
    /// Where the generation's rendered tree is stored.
    pub rendered_dir: PathBuf,
    pub requires_reboot: bool,
}

pub struct ApplyOutcome {
    /// The change is installed but only takes effect after a reboot.
    pub staged: bool,
    pub steps: Vec<String>,
    pub message: String,
}

pub trait Target: Send + Sync {
    fn name(&self) -> &'static str;
    fn describe(&self) -> Value;
    /// Boot-time readiness checks. Returns human-readable notes.
    fn init(&self) -> Result<Vec<String>>;
    fn apply(&self, req: &ApplyRequest<'_>) -> Result<ApplyOutcome>;
    /// Returns the system to a stored generation.
    fn restore(&self, generation: u64, rendered_dir: &Path, requires_reboot: bool) -> Result<Vec<String>>;
    /// System health after an apply.
    fn health(&self, spec: &SystemSpec, generation: u64, artifacts: &[ArtifactInfo]) -> Vec<HealthResult>;
    /// Activates a staged generation (simulated in the sandbox, `systemctl reboot` on NixOS).
    fn reboot(&self) -> Result<String>;
}
