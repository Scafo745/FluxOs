//! On-disk state: immutable generations, plan records, user profiles.
//!
//! ```text
//! <state>/
//!   current                 number of the active generation (atomic rename)
//!   inflight.json           present only while an apply is running
//!   generations/<n>/        spec.json, meta.json, rendered/<artifact tree>
//!   plans/<id>.json
//!   profiles/<id>.json      user-saved and imported profiles
//!   audit/audit.jsonl       hash-chained audit log
//!   secrets/                tokens and signing key (0600)
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use flux_core::SystemSpec;
use flux_core::diff::Change;
use flux_core::policy::{Decision, Issue, OpReport};
use flux_core::render::Artifact;
use flux_core::{RawOp, RiskTier};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::health::HealthResult;
use crate::util::{read_json, safe_relative, write_atomic, write_json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GenStatus {
    Pending,
    Active,
    /// Activated before, replaced by a newer generation. Kept for rollback.
    Superseded,
    /// Staged for the next boot (kernel/boot changes).
    Staged,
    Failed,
    RolledBack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactInfo {
    pub path: String,
    pub sha256: String,
    pub bytes: usize,
}

impl From<&Artifact> for ArtifactInfo {
    fn from(a: &Artifact) -> Self {
        Self { path: a.path.clone(), sha256: a.sha256.clone(), bytes: a.bytes }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationMeta {
    pub number: u64,
    pub parent: Option<u64>,
    pub created_at: String,
    pub plan_id: Option<String>,
    pub summary: String,
    pub tier: RiskTier,
    pub status: GenStatus,
    pub spec_digest: String,
    pub actor: String,
    pub requires_reboot: bool,
    pub theme_name: String,
    pub artifacts: Vec<ArtifactInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    /// Valid, waiting for approval or apply.
    Proposed,
    Approved,
    Applied,
    /// Applied but staged until the next reboot.
    Staged,
    Rejected,
    /// Apply failed and the previous generation was restored.
    RolledBack,
    Failed,
    Blocked,
    Invalid,
    /// Nothing to change.
    NoOp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanKind {
    Change,
    Restore,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Intent {
    /// The user's original request.
    #[serde(default)]
    pub prompt: Option<String>,
    /// Where the plan came from: `agent`, `api`, `mcp`, `cli`.
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approval {
    pub by: String,
    pub role: String,
    pub at: String,
    pub plan_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyResult {
    pub generation: Option<u64>,
    pub status: String,
    pub message: String,
    pub health: Vec<HealthResult>,
    pub rolled_back: bool,
    pub steps: Vec<String>,
    pub duration_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationView {
    pub ok: bool,
    pub blocked: bool,
    pub tier: RiskTier,
    pub operations: Vec<OpReportView>,
    pub issues: Vec<Issue>,
    pub changes: Vec<ChangeView>,
    pub requires_reboot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpReportView {
    pub index: usize,
    pub tool: String,
    pub declared_tier: Option<RiskTier>,
    pub ok: bool,
}

impl From<&OpReport> for OpReportView {
    fn from(o: &OpReport) -> Self {
        Self { index: o.index, tool: o.tool.clone(), declared_tier: o.declared_tier, ok: o.ok }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeView {
    pub path: String,
    pub before: Value,
    pub after: Value,
    pub tier: RiskTier,
    pub summary: String,
    pub requires_reboot: bool,
}

impl From<&Change> for ChangeView {
    fn from(c: &Change) -> Self {
        Self {
            path: c.path.clone(),
            before: c.before.clone(),
            after: c.after.clone(),
            tier: c.tier,
            summary: c.summary.clone(),
            requires_reboot: c.requires_reboot,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewReport {
    pub ok: bool,
    pub artifacts: Vec<ArtifactInfo>,
    pub checks: Vec<HealthResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanRecord {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub status: PlanStatus,
    pub kind: PlanKind,
    pub proposed_by: String,
    pub intent: Intent,
    /// Which engine produced the plan (`heuristic`, `llm:<provider>/<model>`). Untrusted.
    #[serde(default)]
    pub engine: Value,
    /// The model's own description. Untrusted; never used for approval.
    pub ai_summary: Option<String>,
    pub ai_rationale: Option<String>,
    pub operations: Vec<RawOp>,
    pub base_generation: u64,
    pub evaluation: EvaluationView,
    pub decision: Decision,
    /// Plain-language summary derived from the validated diff.
    pub change_summary: String,
    pub plan_hash: String,
    pub approval: Option<Approval>,
    pub preview: Option<PreviewReport>,
    pub result: Option<ApplyResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inflight {
    pub generation: u64,
    pub previous: u64,
    pub plan_id: String,
    pub started_at: String,
}

pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn open(root: &Path) -> Result<Self> {
        for dir in ["generations", "plans", "profiles", "audit", "secrets", "trusted-keys"] {
            fs::create_dir_all(root.join(dir)).with_context(|| format!("create {}", root.join(dir).display()))?;
        }
        Ok(Self { root: root.to_path_buf() })
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn gen_dir(&self, n: u64) -> PathBuf {
        self.root.join("generations").join(n.to_string())
    }

    pub fn rendered_dir(&self, n: u64) -> PathBuf {
        self.gen_dir(n).join("rendered")
    }

    pub fn generation_numbers(&self) -> Result<Vec<u64>> {
        let mut out: Vec<u64> = fs::read_dir(self.root.join("generations"))?
            .flatten()
            .filter_map(|e| e.file_name().to_str().and_then(|s| s.parse().ok()))
            .filter(|n| self.gen_dir(*n).join("meta.json").exists())
            .collect();
        out.sort_unstable();
        Ok(out)
    }

    pub fn next_generation(&self) -> Result<u64> {
        Ok(self.generation_numbers()?.last().copied().unwrap_or(0) + 1)
    }

    pub fn load_meta(&self, n: u64) -> Result<GenerationMeta> {
        read_json(&self.gen_dir(n).join("meta.json"))
    }

    pub fn save_meta(&self, meta: &GenerationMeta) -> Result<()> {
        write_json(&self.gen_dir(meta.number).join("meta.json"), meta)
    }

    pub fn load_spec(&self, n: u64) -> Result<SystemSpec> {
        read_json(&self.gen_dir(n).join("spec.json"))
    }

    /// Writes a complete generation into a temp dir, then renames it in place.
    pub fn write_generation(&self, meta: &GenerationMeta, spec: &SystemSpec, artifacts: &[Artifact]) -> Result<()> {
        let final_dir = self.gen_dir(meta.number);
        if final_dir.exists() {
            bail!("generation {} already exists", meta.number);
        }
        let tmp = self.root.join("generations").join(format!(".{}.tmp", meta.number));
        if tmp.exists() {
            fs::remove_dir_all(&tmp)?;
        }
        fs::create_dir_all(tmp.join("rendered"))?;
        write_json(&tmp.join("spec.json"), spec)?;
        for a in artifacts {
            if !safe_relative(&a.path) {
                bail!("refusing unsafe artifact path {:?}", a.path);
            }
            write_atomic(&tmp.join("rendered").join(&a.path), a.content.as_bytes())?;
        }
        write_json(&tmp.join("meta.json"), meta)?;
        fs::rename(&tmp, &final_dir)?;
        Ok(())
    }

    pub fn read_artifact(&self, n: u64, rel: &str) -> Result<String> {
        if !safe_relative(rel) {
            bail!("invalid artifact path");
        }
        Ok(fs::read_to_string(self.rendered_dir(n).join(rel))?)
    }

    pub fn current(&self) -> Result<Option<u64>> {
        let p = self.root.join("current");
        if !p.exists() {
            return Ok(None);
        }
        Ok(Some(fs::read_to_string(&p)?.trim().parse().context("current generation is not a number")?))
    }

    pub fn set_current(&self, n: u64) -> Result<()> {
        write_atomic(&self.root.join("current"), format!("{n}\n").as_bytes())
    }

    pub fn inflight(&self) -> Result<Option<Inflight>> {
        let p = self.root.join("inflight.json");
        if p.exists() { Ok(Some(read_json(&p)?)) } else { Ok(None) }
    }

    pub fn set_inflight(&self, i: &Inflight) -> Result<()> {
        write_json(&self.root.join("inflight.json"), i)
    }

    pub fn clear_inflight(&self) -> Result<()> {
        let p = self.root.join("inflight.json");
        if p.exists() {
            fs::remove_file(p)?;
        }
        Ok(())
    }

    fn plan_path(&self, id: &str) -> Result<PathBuf> {
        if !id.starts_with("pl_") || !id[3..].chars().all(|c| c.is_ascii_hexdigit()) || id.len() > 64 {
            bail!("invalid plan id");
        }
        Ok(self.root.join("plans").join(format!("{id}.json")))
    }

    pub fn save_plan(&self, plan: &PlanRecord) -> Result<()> {
        write_json(&self.plan_path(&plan.id)?, plan)
    }

    pub fn load_plan(&self, id: &str) -> Result<Option<PlanRecord>> {
        let p = self.plan_path(id)?;
        if p.exists() { Ok(Some(read_json(&p)?)) } else { Ok(None) }
    }

    /// Newest first.
    pub fn list_plans(&self, limit: usize) -> Result<Vec<PlanRecord>> {
        let mut ids: Vec<String> = fs::read_dir(self.root.join("plans"))?
            .flatten()
            .filter_map(|e| e.file_name().to_str().and_then(|s| s.strip_suffix(".json")).map(String::from))
            .collect();
        // uuid v7 ids sort by creation time.
        ids.sort_unstable_by(|a, b| b.cmp(a));
        Ok(ids.iter().take(limit).filter_map(|id| self.load_plan(id).ok().flatten()).collect())
    }

    pub fn user_profile_docs(&self) -> Result<Vec<(flux_core::profiles::ProfileDoc, bool)>> {
        let mut out = Vec::new();
        for e in fs::read_dir(self.root.join("profiles"))?.flatten() {
            if e.path().extension().is_some_and(|x| x == "json") {
                let v: Value = read_json(&e.path())?;
                let imported = v.get("_imported").and_then(Value::as_bool).unwrap_or(false);
                let mut v = v;
                if let Some(o) = v.as_object_mut() {
                    o.remove("_imported");
                }
                out.push((
                    serde_json::from_value(v).with_context(|| format!("parse {}", e.path().display()))?,
                    imported,
                ));
            }
        }
        Ok(out)
    }

    pub fn save_user_profile(&self, doc: &flux_core::profiles::ProfileDoc, imported: bool) -> Result<()> {
        if !flux_core::profiles::valid_id(&doc.id) {
            bail!("invalid profile id");
        }
        let mut v = serde_json::to_value(doc)?;
        v["_imported"] = Value::Bool(imported);
        write_json(&self.root.join("profiles").join(format!("{}.json", doc.id)), &v)
    }
}
