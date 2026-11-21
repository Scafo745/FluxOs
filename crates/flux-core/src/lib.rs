//! FluxOS core.
//!
//! Everything in this crate is pure, deterministic logic with no I/O:
//!
//! * [`spec`]: the declarative system description (a "profile" body).
//! * [`registry`] and [`ops`]: the whitelisted operations the AI may propose,
//!   each with a declared risk tier. Unknown tools are rejected (default-deny).
//! * [`policy`]: validates a proposed plan, simulates it against the current
//!   state, diffs the result and computes the effective risk tier from the
//!   actual change, not from what the model claims.
//! * [`tokens`] and [`color`]: design tokens (W3C DTCG 2025.10) and
//!   contrast-safe palette generation.
//! * [`render`]: deterministic renderers that turn a spec into a NixOS module
//!   and theme artifacts (CSS, GTK, KDE, Hyprland, SVG wallpaper).
//!
//! The daemon (`fluxd`) owns all side effects and is the only component that
//! touches the system. The AI orchestrator never reaches it except through a
//! typed plan that passes this crate's validation.

pub mod catalog;
pub mod color;
pub mod diff;
pub mod hardware;
pub mod nixsyntax;
pub mod ops;
pub mod policy;
pub mod profiles;
pub mod registry;
pub mod render;
pub mod spec;
pub mod tier;
pub mod tokens;

pub use policy::{EvalContext, Evaluation, Issue, RawOp, Severity};
pub use spec::SystemSpec;
pub use tier::RiskTier;

/// Version of the plan/spec schema understood by this build.
pub const SCHEMA_VERSION: &str = "fluxos.spec/v1";

/// SHA-256 of a JSON value's canonical serialization (object keys sorted).
pub fn json_digest(value: &serde_json::Value) -> String {
    use sha2::{Digest, Sha256};
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    format!("sha256:{}", hex::encode(Sha256::digest(&bytes)))
}
