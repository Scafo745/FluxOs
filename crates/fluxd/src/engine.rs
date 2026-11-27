//! Plan lifecycle: propose, approve, apply (snapshot, activate, verify,
//! commit or roll back) and restore. Every step is audited.

use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use flux_core::policy::{self, Decision, EvalContext, Mode, PolicyConfig};
use flux_core::profiles::{ProfileDoc, ProfileSet, ProfileSource};
use flux_core::render::{self, RenderContext};
use flux_core::{RawOp, RiskTier, SystemSpec, json_digest};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::audit::{AuditLog, Record};
use crate::auth::{PairError, Principal, Tokens};
use crate::events::EventBus;
use crate::health::{self, HealthResult, HealthStatus};
use crate::signing::{self, Signer};
use crate::store::*;
use crate::targets::{ApplyRequest, Target};
use crate::util::{new_id, now, unix_now};

#[derive(Debug, Clone, Serialize)]
pub struct EngineError {
    #[serde(skip)]
    pub status: u16,
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl EngineError {
    pub fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self { status, code, message: message.into(), details: None }
    }
    pub fn bad_request(m: impl Into<String>) -> Self {
        Self::new(400, "bad_request", m)
    }
    pub fn forbidden(m: impl Into<String>) -> Self {
        Self::new(403, "forbidden", m)
    }
    pub fn not_found(m: impl Into<String>) -> Self {
        Self::new(404, "not_found", m)
    }
    pub fn conflict(m: impl Into<String>) -> Self {
        Self::new(409, "conflict", m)
    }
    pub fn internal(e: impl std::fmt::Display) -> Self {
        Self::new(500, "internal", format!("{e:#}"))
    }
    pub fn with_details(mut self, d: Value) -> Self {
        self.details = Some(d);
        self
    }
}

impl From<anyhow::Error> for EngineError {
    fn from(e: anyhow::Error) -> Self {
        Self::internal(e)
    }
}

pub type EResult<T> = Result<T, EngineError>;

#[derive(Debug, Clone, Serialize)]
pub struct BootStage {
    pub id: String,
    pub title: String,
    pub status: String,
    pub detail: String,
    pub duration_ms: u128,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct BootReport {
    pub version: String,
    pub boot_id: String,
    pub started_at: String,
    pub ready: bool,
    pub duration_ms: u128,
    pub target: String,
    pub generation: u64,
    pub stages: Vec<BootStage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Fault {
    /// The next apply reports a failed health check (demonstrates auto-rollback).
    Health,
    /// The next apply fails while activating.
    Apply,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProposeRequest {
    #[serde(default)]
    pub intent: Intent,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub rationale: Option<String>,
    #[serde(default)]
    pub engine: Value,
    pub operations: Vec<RawOp>,
}

pub struct Engine {
    pub version: &'static str,
    pub started: Instant,
    pub mode: Mode,
    pub policy: PolicyConfig,
    pub dev: bool,
    pub store: Store,
    pub audit: AuditLog,
    pub events: EventBus,
    pub tokens: Tokens,
    pub signer: Signer,
    pub hardware: flux_core::hardware::HardwareInfo,
    pub target: Arc<dyn Target>,
    pub profiles: RwLock<ProfileSet>,
    pub boot: RwLock<BootReport>,
    pub degraded: RwLock<Vec<String>>,
    /// Random id of this daemon instance; lets clients detect restarts.
    pub boot_id: String,
    /// flux-agent address as seen by browsers.
    pub agent_url: String,
    /// Extra accepted Host header names (loopback is always accepted).
    pub allowed_hosts: Vec<String>,
    /// The console user's flux-session (terminal, files, apps), if any.
    pub session: crate::session::SessionLink,
    current: RwLock<u64>,
    apply_lock: tokio::sync::Mutex<()>,
    fault: Mutex<Option<Fault>>,
}

/// Everything `Engine::new` needs; assembled by the boot sequence.
pub struct EngineParts {
    pub mode: Mode,
    pub policy: PolicyConfig,
    pub dev: bool,
    pub store: Store,
    pub audit: AuditLog,
    pub events: EventBus,
    pub tokens: Tokens,
    pub signer: Signer,
    pub hardware: flux_core::hardware::HardwareInfo,
    pub target: Arc<dyn Target>,
    pub profiles: ProfileSet,
    pub current: u64,
    pub boot_id: String,
    pub agent_url: String,
    pub allowed_hosts: Vec<String>,
    pub session: crate::session::SessionLink,
}

fn tier_u8(t: RiskTier) -> Option<u8> {
    Some(t.as_u8())
}

fn change_summary(changes: &[ChangeView]) -> String {
    if changes.is_empty() {
        return "No changes.".into();
    }
    let mut lines: Vec<String> = changes.iter().take(12).map(|c| c.summary.clone()).collect();
    if changes.len() > 12 {
        lines.push(format!("…and {} more changes", changes.len() - 12));
    }
    lines.join("\n")
}

impl Engine {
    pub fn new(p: EngineParts) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            started: Instant::now(),
            mode: p.mode,
            policy: p.policy,
            dev: p.dev,
            store: p.store,
            audit: p.audit,
            events: p.events,
            tokens: p.tokens,
            signer: p.signer,
            hardware: p.hardware,
            target: p.target,
            profiles: RwLock::new(p.profiles),
            boot: RwLock::new(BootReport::default()),
            degraded: RwLock::new(Vec::new()),
            boot_id: p.boot_id,
            agent_url: p.agent_url,
            allowed_hosts: p.allowed_hosts,
            session: p.session,
            current: RwLock::new(p.current),
            apply_lock: tokio::sync::Mutex::new(()),
            fault: Mutex::new(None),
        }
    }

    pub fn current_generation(&self) -> u64 {
        *self.current.read().expect("current lock")
    }

    pub fn current_spec(&self) -> EResult<SystemSpec> {
        Ok(self.store.load_spec(self.current_generation())?)
    }

    pub fn degraded_reasons(&self) -> Vec<String> {
        self.degraded.read().expect("degraded lock").clone()
    }

    pub fn set_degraded(&self, reason: String) {
        self.degraded.write().expect("degraded lock").push(reason);
    }

    pub fn set_fault(&self, f: Option<Fault>) {
        *self.fault.lock().expect("fault lock") = f;
    }

    fn take_fault(&self) -> Option<Fault> {
        self.fault.lock().expect("fault lock").take()
    }

    pub(crate) fn audit(&self, r: Record<'_>) {
        if let Err(e) = self.audit.append(r) {
            tracing::error!("audit append failed: {e:#}");
            self.set_degraded(format!("audit append failed: {e}"));
        }
    }

    /// Evaluates operations against `base` with the live policy context.
    pub fn evaluate(&self, base: &SystemSpec, ops: &[RawOp]) -> policy::Evaluation {
        let profiles = self.profiles.read().expect("profiles lock");
        let lookup = |n: u64| self.store.load_spec(n).ok();
        let ctx = EvalContext {
            profiles: &profiles,
            hardware: &self.hardware,
            generation: &lookup,
            disabled_tools: &self.policy.disabled_tools,
        };
        policy::evaluate(base, ops, &ctx)
    }

    fn plan_hash(base: u64, ops: &[RawOp], candidate: Option<&SystemSpec>) -> String {
        json_digest(&json!({
            "base_generation": base,
            "operations": ops,
            "candidate": candidate.map(SystemSpec::digest),
        }))
    }

    /// Validates and records a plan. With `dry_run`, nothing is stored or audited.
    pub fn propose(&self, who: &Principal, req: ProposeRequest, kind: PlanKind, dry_run: bool) -> EResult<PlanRecord> {
        let base = self.current_generation();
        let current = self.current_spec()?;
        let eval = self.evaluate(&current, &req.operations);
        let changes: Vec<ChangeView> = eval.changes.iter().map(ChangeView::from).collect();
        let status = if eval.blocked {
            PlanStatus::Blocked
        } else if !eval.ok {
            PlanStatus::Invalid
        } else if eval.no_changes() {
            PlanStatus::NoOp
        } else {
            PlanStatus::Proposed
        };
        let decision = if eval.ok { policy::decide(eval.tier, self.mode) } else { Decision::Blocked };

        let preview = match (&eval.candidate, status) {
            (Some(candidate), PlanStatus::Proposed) => {
                let next = self.store.next_generation()?;
                let artifacts = render::render_all(candidate, &RenderContext { generation: next, plan_id: None });
                let checks = health::artifact_checks(candidate, &artifacts);
                Some(PreviewReport {
                    ok: !health::any_failed(&checks),
                    artifacts: artifacts.iter().map(ArtifactInfo::from).collect(),
                    checks,
                })
            }
            _ => None,
        };

        let id = new_id("pl");
        let ts = now();
        let plan = PlanRecord {
            id: id.clone(),
            created_at: ts.clone(),
            updated_at: ts,
            status,
            kind,
            proposed_by: who.id.clone(),
            intent: req.intent,
            engine: req.engine,
            ai_summary: req.summary.map(|s| s.chars().take(2000).collect()),
            ai_rationale: req.rationale.map(|s| s.chars().take(4000).collect()),
            plan_hash: Self::plan_hash(base, &req.operations, eval.candidate.as_ref()),
            operations: req.operations,
            base_generation: base,
            change_summary: change_summary(&changes),
            evaluation: EvaluationView {
                ok: eval.ok,
                blocked: eval.blocked,
                tier: eval.tier,
                operations: eval.operations.iter().map(OpReportView::from).collect(),
                issues: eval.issues.clone(),
                changes,
                requires_reboot: eval.requires_reboot,
            },
            decision,
            approval: None,
            preview,
            result: None,
        };

        if !dry_run {
            self.store.save_plan(&plan)?;
            let kind = match status {
                PlanStatus::Blocked => "plan.blocked",
                PlanStatus::Invalid => "plan.invalid",
                PlanStatus::NoOp => "plan.noop",
                _ => "plan.proposed",
            };
            let errors: Vec<&str> =
                plan.evaluation.issues.iter().filter(|i| i.is_error()).map(|i| i.message.as_str()).collect();
            self.audit(Record {
                kind,
                actor: &who.id,
                plan_id: Some(&plan.id),
                generation: Some(base),
                tier: tier_u8(plan.evaluation.tier),
                summary: match status {
                    PlanStatus::Blocked | PlanStatus::Invalid => format!("Rejected: {}", errors.join("; ")),
                    _ => plan.change_summary.replace('\n', "; "),
                },
                details: json!({
                    "prompt": plan.intent.prompt,
                    "source": plan.intent.source,
                    "engine": plan.engine,
                    "operations": plan.operations,
                    "decision": plan.decision,
                    "plan_hash": plan.plan_hash,
                }),
            });
            self.events.emit(
                kind,
                json!({
                    "plan_id": plan.id,
                    "tier": plan.evaluation.tier,
                    "decision": plan.decision,
                    "summary": plan.change_summary,
                    "status": plan.status,
                    "prompt": plan.intent.prompt,
                    "source": plan.intent.source,
                    "session_id": plan.intent.session_id,
                }),
            );
        }
        Ok(plan)
    }

    pub fn get_plan(&self, id: &str) -> EResult<PlanRecord> {
        self.store
            .load_plan(id)
            .map_err(|e| EngineError::bad_request(e.to_string()))?
            .ok_or_else(|| EngineError::not_found(format!("plan {id} not found")))
    }

    pub fn reject(&self, who: &Principal, id: &str, reason: Option<String>) -> EResult<PlanRecord> {
        if !who.is_human() {
            return Err(EngineError::forbidden("only a person can reject a plan"));
        }
        let mut plan = self.get_plan(id)?;
        if !matches!(plan.status, PlanStatus::Proposed | PlanStatus::Approved) {
            return Err(EngineError::conflict(format!("plan is {:?}", plan.status)));
        }
        plan.status = PlanStatus::Rejected;
        plan.updated_at = now();
        self.store.save_plan(&plan)?;
        self.audit(Record {
            kind: "plan.rejected",
            actor: &who.id,
            plan_id: Some(id),
            generation: Some(plan.base_generation),
            tier: tier_u8(plan.evaluation.tier),
            summary: reason.clone().unwrap_or_else(|| "Rejected by user".into()),
            details: json!({ "reason": reason }),
        });
        self.events.emit("plan.rejected", json!({ "plan_id": id }));
        Ok(plan)
    }

    pub async fn approve(&self, who: &Principal, id: &str, plan_hash: &str, apply: bool) -> EResult<PlanRecord> {
        let mut plan = self.get_plan(id)?;
        if plan.status != PlanStatus::Proposed {
            return Err(EngineError::conflict(format!("plan is {:?}, not proposed", plan.status)));
        }
        let tier = plan.evaluation.tier;
        if !who.can_approve(tier, self.mode) {
            return Err(EngineError::forbidden(format!(
                "{} ({:?}) cannot approve a tier {} change in {:?} mode",
                who.id,
                who.role,
                tier.as_u8(),
                self.mode
            )));
        }
        if plan_hash != plan.plan_hash {
            return Err(EngineError::conflict("plan_hash does not match: review the current plan before approving"));
        }
        plan.approval = Some(Approval {
            by: who.id.clone(),
            role: format!("{:?}", who.role).to_lowercase(),
            at: now(),
            plan_hash: plan.plan_hash.clone(),
        });
        plan.status = PlanStatus::Approved;
        plan.updated_at = now();
        self.store.save_plan(&plan)?;
        self.audit(Record {
            kind: "plan.approved",
            actor: &who.id,
            plan_id: Some(id),
            generation: Some(plan.base_generation),
            tier: tier_u8(tier),
            summary: plan.change_summary.replace('\n', "; "),
            details: json!({ "plan_hash": plan.plan_hash, "role": who.role }),
        });
        self.events.emit("plan.approved", json!({ "plan_id": id, "by": who.id }));
        if apply { self.apply(who, id).await } else { Ok(plan) }
    }

    fn check_freeze(&self, tier: RiskTier) -> EResult<()> {
        let t = unix_now();
        for w in &self.policy.freeze_windows {
            if w.blocks(t, tier) {
                return Err(EngineError::new(
                    423,
                    "change_freeze",
                    format!(
                        "change freeze {:?} is active ({}-{}); tier {} changes are deferred",
                        w.name,
                        w.start,
                        w.end,
                        tier.as_u8()
                    ),
                ));
            }
        }
        Ok(())
    }

    /// Applies a plan: authorize, re-validate, snapshot, activate, verify,
    /// then commit or roll back.
    pub async fn apply(&self, who: &Principal, id: &str) -> EResult<PlanRecord> {
        let _guard = self.apply_lock.lock().await;
        let mut plan = self.get_plan(id)?;
        if !matches!(plan.status, PlanStatus::Proposed | PlanStatus::Approved) {
            return Err(EngineError::conflict(format!(
                "plan is {:?}; only proposed or approved plans can be applied",
                plan.status
            )));
        }
        if let Ok(created) = chrono::DateTime::parse_from_rfc3339(&plan.created_at)
            && chrono::Utc::now().signed_duration_since(created) > chrono::Duration::hours(24)
        {
            return Err(EngineError::conflict("plan expired after 24h; propose it again"));
        }
        let approved_tier = plan.evaluation.tier;
        if approved_tier == RiskTier::Blocked {
            return Err(EngineError::forbidden("blocked plans can never be applied"));
        }

        // Permissions are re-checked on every call.
        if !who.can_apply_without_approval(approved_tier) {
            if !who.is_human() {
                return Err(EngineError::forbidden(format!(
                    "tier {} changes need a person to approve and apply them",
                    approved_tier.as_u8()
                )));
            }
            match &plan.approval {
                Some(a) if a.plan_hash == plan.plan_hash => {}
                _ => {
                    return Err(EngineError::forbidden(format!(
                        "tier {} plan has no valid approval",
                        approved_tier.as_u8()
                    ))
                    .with_details(json!({ "decision": plan.decision, "plan_hash": plan.plan_hash })));
                }
            }
            if !who.can_approve(approved_tier, self.mode) {
                return Err(EngineError::forbidden(format!(
                    "{} cannot apply tier {} changes",
                    who.id,
                    approved_tier.as_u8()
                )));
            }
        }
        self.check_freeze(approved_tier)?;
        if approved_tier >= RiskTier::Medium && !self.degraded_reasons().is_empty() {
            return Err(EngineError::new(
                503,
                "degraded",
                format!(
                    "daemon is degraded ({}); only tier 0-1 changes are allowed",
                    self.degraded_reasons().join("; ")
                ),
            ));
        }

        // Re-validate against the live state; the world may have moved on.
        let previous = self.current_generation();
        let current = self.current_spec()?;
        let eval = self.evaluate(&current, &plan.operations);
        if !eval.ok {
            return Err(EngineError::conflict("plan no longer validates against the current system; propose it again")
                .with_details(json!({ "issues": eval.issues })));
        }
        if eval.tier > approved_tier {
            return Err(EngineError::conflict(format!(
                "plan now needs tier {} (approved at tier {}); propose it again",
                eval.tier.as_u8(),
                approved_tier.as_u8()
            )));
        }
        let Some(candidate) = eval.candidate.clone() else {
            return Err(EngineError::conflict("plan produced no candidate state"));
        };
        if eval.changes.is_empty() {
            plan.status = PlanStatus::NoOp;
            plan.updated_at = now();
            self.store.save_plan(&plan)?;
            return Ok(plan);
        }

        let started = Instant::now();
        let n = self.store.next_generation()?;
        let artifacts = render::render_all(&candidate, &RenderContext { generation: n, plan_id: Some(id) });
        let pre = health::artifact_checks(&candidate, &artifacts);
        if health::any_failed(&pre) {
            plan.status = PlanStatus::Failed;
            plan.result = Some(ApplyResult {
                generation: None,
                status: "failed".into(),
                message: "pre-apply checks failed; nothing was changed".into(),
                health: pre,
                rolled_back: false,
                steps: vec![],
                duration_ms: started.elapsed().as_millis(),
            });
            plan.updated_at = now();
            self.store.save_plan(&plan)?;
            return Ok(plan);
        }

        let mut meta = GenerationMeta {
            number: n,
            parent: Some(previous),
            created_at: now(),
            plan_id: Some(id.to_string()),
            summary: change_summary(&eval.changes.iter().map(ChangeView::from).collect::<Vec<_>>()).replace('\n', "; "),
            tier: eval.tier,
            status: GenStatus::Pending,
            spec_digest: candidate.digest(),
            actor: who.id.clone(),
            requires_reboot: eval.requires_reboot,
            theme_name: candidate.design.theme_name.clone(),
            artifacts: artifacts.iter().map(ArtifactInfo::from).collect(),
        };
        self.store.write_generation(&meta, &candidate, &artifacts)?;
        self.store.set_inflight(&Inflight { generation: n, previous, plan_id: id.to_string(), started_at: now() })?;
        self.audit(Record {
            kind: "apply.started",
            actor: &who.id,
            plan_id: Some(id),
            generation: Some(n),
            tier: tier_u8(eval.tier),
            summary: format!("Snapshot: generation {previous} kept; building generation {n}"),
            details: json!({ "previous": previous, "approval": plan.approval, "artifacts": meta.artifacts.len() }),
        });
        // Lets a UI recognize the plans it started (and skip duplicate notifications).
        let origin = json!({ "source": plan.intent.source, "session_id": plan.intent.session_id });
        self.events.emit(
            "apply.started",
            json!({ "plan_id": id, "generation": n, "previous": previous, "tier": eval.tier, "origin": origin }),
        );

        let fault = self.take_fault();
        let target = self.target.clone();
        let rendered = self.store.rendered_dir(n);
        let requires_reboot = eval.requires_reboot;
        let spec_for_apply = candidate.clone();
        let artifacts_for_apply = artifacts.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            if fault == Some(Fault::Apply) {
                anyhow::bail!("injected fault: activation failed");
            }
            target.apply(&ApplyRequest {
                generation: n,
                previous,
                spec: &spec_for_apply,
                artifacts: &artifacts_for_apply,
                rendered_dir: rendered,
                requires_reboot,
            })
        })
        .await
        .map_err(EngineError::internal)?;

        let outcome = match outcome {
            Ok(o) => o,
            Err(e) => {
                let restore = self.restore_target(previous).await;
                meta.status = GenStatus::Failed;
                self.store.save_meta(&meta)?;
                self.store.clear_inflight()?;
                plan.status = PlanStatus::Failed;
                plan.result = Some(ApplyResult {
                    generation: Some(n),
                    status: "failed".into(),
                    message: format!("activation failed: {e:#}. Generation {previous} restored."),
                    health: vec![],
                    rolled_back: restore.is_ok(),
                    steps: restore.unwrap_or_else(|e| vec![format!("restore failed: {e:#}")]),
                    duration_ms: started.elapsed().as_millis(),
                });
                plan.updated_at = now();
                self.store.save_plan(&plan)?;
                self.audit(Record {
                    kind: "apply.failed",
                    actor: &who.id,
                    plan_id: Some(id),
                    generation: Some(n),
                    tier: tier_u8(eval.tier),
                    summary: format!("Activation failed; generation {previous} restored"),
                    details: json!({ "error": format!("{e:#}") }),
                });
                self.events.emit(
                    "apply.failed",
                    json!({ "plan_id": id, "generation": n, "restored": previous, "error": format!("{e:#}"), "origin": origin }),
                );
                return Ok(plan);
            }
        };
        for step in &outcome.steps {
            self.events.emit("apply.step", json!({ "plan_id": id, "generation": n, "step": step }));
        }

        // Verify.
        let target = self.target.clone();
        let spec_for_health = candidate.clone();
        let infos = meta.artifacts.clone();
        let mut checks = tokio::task::spawn_blocking(move || target.health(&spec_for_health, n, &infos))
            .await
            .map_err(EngineError::internal)?;
        if fault == Some(Fault::Health) {
            checks.push(HealthResult::fail(
                "fault.injected",
                "Injected fault",
                "developer fault injection: simulated failed health check",
            ));
        }
        for c in &checks {
            self.events.emit("apply.health", json!({ "plan_id": id, "generation": n, "check": c }));
        }

        if health::any_failed(&checks) {
            let restore = self.restore_target(previous).await;
            meta.status = GenStatus::RolledBack;
            self.store.save_meta(&meta)?;
            self.store.clear_inflight()?;
            let failed: Vec<String> = checks
                .iter()
                .filter(|c| c.status == HealthStatus::Fail)
                .map(|c| format!("{}: {}", c.name, c.detail))
                .collect();
            let rolled_back = restore.is_ok();
            let mut steps = outcome.steps.clone();
            match restore {
                Ok(s) => steps.extend(s),
                Err(e) => steps.push(format!("restore failed: {e:#}")),
            }
            plan.status = PlanStatus::RolledBack;
            plan.result = Some(ApplyResult {
                generation: Some(n),
                status: "rolled_back".into(),
                message: format!(
                    "Health checks failed ({}); automatically rolled back to generation {previous}.",
                    failed.join("; ")
                ),
                health: checks,
                rolled_back,
                steps,
                duration_ms: started.elapsed().as_millis(),
            });
            plan.updated_at = now();
            self.store.save_plan(&plan)?;
            self.audit(Record {
                kind: "apply.rolled_back",
                actor: "fluxd",
                plan_id: Some(id),
                generation: Some(n),
                tier: tier_u8(eval.tier),
                summary: format!("Health check failed; rolled back to generation {previous}"),
                details: json!({ "failed": failed }),
            });
            self.events.emit(
                "apply.rolled_back",
                json!({ "plan_id": id, "generation": n, "restored": previous, "failed": failed, "origin": origin }),
            );
            return Ok(plan);
        }

        // Commit.
        self.store.set_current(n)?;
        *self.current.write().expect("current lock") = n;
        if let Ok(mut prev_meta) = self.store.load_meta(previous) {
            prev_meta.status = GenStatus::Superseded;
            let _ = self.store.save_meta(&prev_meta);
        }
        meta.status = if outcome.staged { GenStatus::Staged } else { GenStatus::Active };
        self.store.save_meta(&meta)?;
        self.store.clear_inflight()?;
        plan.status = if outcome.staged { PlanStatus::Staged } else { PlanStatus::Applied };
        plan.result = Some(ApplyResult {
            generation: Some(n),
            status: if outcome.staged { "staged".into() } else { "applied".into() },
            message: outcome.message.clone(),
            health: checks,
            rolled_back: false,
            steps: outcome.steps,
            duration_ms: started.elapsed().as_millis(),
        });
        plan.updated_at = now();
        self.store.save_plan(&plan)?;
        self.audit(Record {
            kind: "apply.committed",
            actor: &who.id,
            plan_id: Some(id),
            generation: Some(n),
            tier: tier_u8(eval.tier),
            summary: outcome.message.clone(),
            details: json!({
                "prompt": plan.intent.prompt,
                "operations": plan.operations,
                "changes": plan.evaluation.changes.len(),
                "approved_by": plan.approval.as_ref().map(|a| &a.by),
                "previous": previous,
                "staged": outcome.staged,
            }),
        });
        self.events.emit(
            "generation.switched",
            json!({
                "generation": n,
                "previous": previous,
                "plan_id": id,
                "staged": outcome.staged,
                "theme": candidate.design.theme_name,
                "summary": meta.summary,
                "origin": origin,
            }),
        );
        Ok(plan)
    }

    async fn restore_target(&self, generation: u64) -> anyhow::Result<Vec<String>> {
        let target = self.target.clone();
        let dir = self.store.rendered_dir(generation);
        let reboot = self.store.load_meta(generation).map(|m| m.requires_reboot).unwrap_or(false);
        tokio::task::spawn_blocking(move || target.restore(generation, &dir, reboot)).await?
    }

    /// Undo: propose restoring a generation (default: the current one's parent).
    /// A person who may approve the resulting tier gets it applied immediately.
    pub async fn rollback(
        &self,
        who: &Principal,
        to: Option<u64>,
        prompt: Option<String>,
        session_id: Option<String>,
    ) -> EResult<PlanRecord> {
        let current = self.current_generation();
        let to = match to {
            Some(t) => t,
            None => self
                .store
                .load_meta(current)?
                .parent
                .ok_or_else(|| EngineError::conflict("the current generation has no parent to roll back to"))?,
        };
        if to == current {
            return Err(EngineError::conflict(format!("generation {to} is already active")));
        }
        let meta =
            self.store.load_meta(to).map_err(|_| EngineError::not_found(format!("generation {to} not found")))?;
        if matches!(meta.status, GenStatus::Failed | GenStatus::RolledBack | GenStatus::Pending) {
            return Err(EngineError::conflict(format!("generation {to} never became healthy ({:?})", meta.status)));
        }
        let req = ProposeRequest {
            intent: Intent {
                prompt: prompt.or_else(|| Some(format!("Roll back to generation {to}"))),
                source: Some("rollback".into()),
                session_id: session_id
                    .filter(|s| s.len() <= 64 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')),
            },
            summary: Some(format!("Restore generation {to}")),
            rationale: None,
            engine: json!({ "kind": "rollback" }),
            operations: vec![RawOp {
                tool: "generation.restore".into(),
                params: json!({ "generation": to }),
                reason: Some("undo".into()),
            }],
        };
        let plan = self.propose(who, req, PlanKind::Restore, false)?;
        match plan.decision {
            Decision::AutoApply if plan.status == PlanStatus::Proposed => self.apply(who, &plan.id).await,
            Decision::Confirm | Decision::AdminApproval if who.can_approve(plan.evaluation.tier, self.mode) => {
                let hash = plan.plan_hash.clone();
                self.approve(who, &plan.id, &hash, true).await
            }
            _ => Ok(plan),
        }
    }

    /// Activates a staged generation. Simulated on the sandbox target.
    pub async fn reboot(&self, who: &Principal) -> EResult<Value> {
        if who.role != crate::auth::Role::Admin {
            return Err(EngineError::forbidden("only the device owner or an administrator can reboot"));
        }
        let n = self.current_generation();
        let target = self.target.clone();
        let msg = tokio::task::spawn_blocking(move || target.reboot()).await.map_err(EngineError::internal)??;
        let mut meta = self.store.load_meta(n)?;
        let was_staged = meta.status == GenStatus::Staged;
        if was_staged && self.target.name() == "sandbox" {
            meta.status = GenStatus::Active;
            self.store.save_meta(&meta)?;
        }
        self.audit(Record {
            kind: "system.reboot",
            actor: &who.id,
            plan_id: None,
            generation: Some(n),
            tier: Some(RiskTier::High.as_u8()),
            summary: msg.clone(),
            details: json!({ "staged_activated": was_staged }),
        });
        self.events.emit("system.reboot", json!({ "generation": n, "message": msg, "staged_activated": was_staged }));
        if self.target.name() == "sandbox" {
            self.replay_simulated_boot(n, &meta).await?;
        }
        Ok(json!({ "generation": n, "message": msg, "staged_activated": was_staged }))
    }

    /// Shuts the machine down (real targets only). Audited before it happens.
    pub async fn poweroff(&self, who: &Principal) -> EResult<Value> {
        if who.role != crate::auth::Role::Admin {
            return Err(EngineError::forbidden("only the device owner or an administrator can shut down"));
        }
        if self.target.name() == "sandbox" {
            return Err(EngineError::new(409, "not_supported", "the sandbox target has no machine to shut down"));
        }
        let n = self.current_generation();
        self.audit(Record {
            kind: "system.poweroff",
            actor: &who.id,
            plan_id: None,
            generation: Some(n),
            tier: Some(RiskTier::Medium.as_u8()),
            summary: "Shut down from the FluxOS UI".into(),
            details: json!({}),
        });
        self.events.emit("system.poweroff", json!({ "generation": n }));
        let target = self.target.clone();
        let msg = tokio::task::spawn_blocking(move || target.poweroff()).await.map_err(EngineError::internal)??;
        Ok(json!({ "generation": n, "message": msg }))
    }

    /// Emits the boot stages of a simulated reboot so UIs can show the new
    /// generation coming up. The health stage runs the real checks.
    async fn replay_simulated_boot(&self, n: u64, meta: &GenerationMeta) -> EResult<()> {
        let started = Instant::now();
        let spec = self.current_spec()?;
        let kernel = flux_core::catalog::kernel(spec.kernel.variant);
        let sched = flux_core::catalog::scheduler(spec.kernel.sched_ext);
        let older = self.store.generation_numbers()?.iter().filter(|g| **g < n).count();
        self.events.emit(
            "boot.begin",
            json!({ "version": self.version, "target": self.target.name(), "boot_id": self.boot_id, "reboot": true }),
        );
        let session = match spec.session.mode {
            flux_core::spec::SessionMode::Standard => {
                format!("{:?} desktop · {:?} style", spec.desktop.environment, spec.desktop.style).to_lowercase()
            }
            mode => format!("{mode:?} session → {}", spec.session.kiosk_url.as_deref().unwrap_or("-")).to_lowercase(),
        };
        let stage = |id: &str, title: &str, status: &str, detail: String| json!({ "id": id, "title": title, "status": status, "detail": detail, "duration_ms": 0, "reboot": true });
        let stages = [
            stage("firmware", "Firmware", "ok", "POST complete".into()),
            stage(
                "bootloader",
                "Bootloader",
                "ok",
                format!("fluxos-gen-{n} selected · {older} older generations kept"),
            ),
            stage("kernel", "Kernel", "ok", format!("{} · {}", kernel.label, kernel.version_hint)),
            stage(
                "scheduler",
                "CPU scheduler",
                "ok",
                match sched.binary {
                    Some(bin) => format!("{bin} loaded via sched_ext ({})", sched.maturity),
                    None => "kernel default (EEVDF)".into(),
                },
            ),
            stage("session", "Session", "ok", session),
        ];
        for s in stages {
            self.events.emit("boot.stage", s);
        }
        let target = self.target.clone();
        let infos = meta.artifacts.clone();
        let checks = tokio::task::spawn_blocking(move || target.health(&spec, n, &infos))
            .await
            .map_err(EngineError::internal)?;
        let passed = checks.iter().filter(|c| c.status == HealthStatus::Pass).count();
        let failed = checks.iter().any(|c| c.status == HealthStatus::Fail);
        self.events.emit(
            "boot.stage",
            stage(
                "health",
                "Health checks",
                if failed { "fail" } else { "ok" },
                format!("{passed}/{} passed", checks.len()),
            ),
        );
        self.events.emit(
            "boot.ready",
            json!({ "generation": n, "reboot": true, "duration_ms": started.elapsed().as_millis(), "boot_id": self.boot_id, "degraded": self.degraded_reasons() }),
        );
        Ok(())
    }

    pub fn generations(&self) -> EResult<Vec<GenerationMeta>> {
        let mut out = Vec::new();
        for n in self.store.generation_numbers()?.into_iter().rev() {
            out.push(self.store.load_meta(n)?);
        }
        Ok(out)
    }

    /// Exchanges a one-time pairing code for a session token.
    pub fn pair(&self, code: &str, name: &str) -> EResult<Value> {
        match self.tokens.pair(code, name, self.mode) {
            Ok(p) => {
                self.audit(Record {
                    kind: "session.paired",
                    actor: &p.principal.id,
                    plan_id: None,
                    generation: None,
                    tier: None,
                    summary: format!("Paired \"{}\" as {}", p.session.name, p.principal.id),
                    details: json!({ "session": p.session.public() }),
                });
                self.events.emit("session.paired", json!({ "session": p.session.public() }));
                tracing::info!(target: "fluxd::pairing", "screen paired; next pairing code {}", self.tokens.pairing_code());
                Ok(json!({
                    "token": p.token,
                    "session": p.session.public(),
                    "principal": p.principal,
                    "mode": self.mode,
                    "intent_token": self.tokens.intent_token(),
                    "agent_url": self.agent_url,
                }))
            }
            Err(PairError::WrongCode { attempts_left }) => Err(EngineError::new(
                403,
                "wrong_code",
                format!("That code is not valid. {attempts_left} attempts left before it changes."),
            )
            .with_details(json!({ "attempts_left": attempts_left }))),
            Err(PairError::Locked { retry_after_s, rotated }) => {
                if rotated {
                    self.audit(Record {
                        kind: "pairing.locked",
                        actor: "unknown",
                        plan_id: None,
                        generation: None,
                        tier: None,
                        summary: format!(
                            "{} wrong pairing codes; code rotated and pairing paused for {retry_after_s}s",
                            crate::auth::PAIR_ATTEMPTS
                        ),
                        details: json!({}),
                    });
                    tracing::warn!(
                        target: "fluxd::pairing",
                        "too many wrong pairing codes; new code {} (pairing paused {retry_after_s}s)",
                        self.tokens.pairing_code()
                    );
                }
                Err(EngineError::new(
                    429,
                    "pairing_locked",
                    format!("Too many wrong codes. Try again in {retry_after_s}s with the new code."),
                )
                .with_details(json!({ "retry_after_s": retry_after_s })))
            }
            Err(PairError::Internal(e)) => Err(EngineError::internal(e)),
        }
    }

    /// Revokes a paired screen. Anyone may revoke their own session; admins any.
    pub fn revoke_session(&self, who: &Principal, id: &str) -> EResult<Value> {
        let own = who.session.as_deref() == Some(id);
        if !own && !who.can_manage_sessions() {
            return Err(EngineError::forbidden("only an administrator can revoke other screens"));
        }
        let removed =
            self.tokens.revoke(id)?.ok_or_else(|| EngineError::not_found(format!("session {id} not found")))?;
        self.audit(Record {
            kind: "session.revoked",
            actor: &who.id,
            plan_id: None,
            generation: None,
            tier: None,
            summary: format!("Revoked \"{}\"{}", removed.name, if own { " (signed out)" } else { "" }),
            details: json!({ "session": removed.public() }),
        });
        self.events.emit("session.revoked", json!({ "session": removed.public() }));
        Ok(json!({ "revoked": removed.public() }))
    }

    pub fn save_profile(&self, who: &Principal, id: &str, name: &str, summary: Option<String>) -> EResult<Value> {
        if !who.can_manage_profiles() {
            return Err(EngineError::forbidden("only a person can save profiles"));
        }
        let spec = self.current_spec()?;
        let doc = ProfileDoc {
            id: id.to_string(),
            name: name.to_string(),
            version: "1.0.0".into(),
            vertical: None,
            summary: summary.unwrap_or_else(|| format!("Saved from generation {}", self.current_generation())),
            description: String::new(),
            tags: vec!["custom".into()],
            status: "stable".into(),
            compliance_review_required: false,
            integrations: vec![],
            style: None,
            palette: None,
            overlay: serde_json::to_value(&spec).map_err(EngineError::internal)?,
        };
        let summary = {
            let mut profiles = self.profiles.write().expect("profiles lock");
            profiles.insert(doc.clone(), ProfileSource::User).map_err(EngineError::bad_request)?.summary_json()
        };
        self.store.save_user_profile(&doc, false)?;
        self.audit(Record {
            kind: "profile.saved",
            actor: &who.id,
            plan_id: None,
            generation: Some(self.current_generation()),
            tier: None,
            summary: format!("Saved profile {id}"),
            details: json!({ "id": id, "spec_digest": spec.digest() }),
        });
        Ok(summary)
    }

    /// Signed, shareable bundle for a profile.
    pub fn export_profile(&self, id: &str) -> EResult<Value> {
        let profiles = self.profiles.read().expect("profiles lock");
        let p = profiles.get(id).ok_or_else(|| EngineError::not_found(format!("profile {id} not found")))?;
        let body = json!({
            "format": "fluxos-profile-bundle/v1",
            "profile": p.doc,
            "spec_digest": p.spec.digest(),
            "signed_at": now(),
            "publisher": { "fingerprint": self.signer.fingerprint(), "public_key": self.signer.public_key() },
        });
        let signature = self.signer.sign(&body);
        let mut bundle = body;
        bundle["signature"] = json!(signature);
        Ok(bundle)
    }

    pub fn import_profile(&self, who: &Principal, mut bundle: Value) -> EResult<Value> {
        if !who.can_manage_profiles() {
            return Err(EngineError::forbidden("only a person can import profiles"));
        }
        let signature = bundle
            .as_object_mut()
            .and_then(|o| o.remove("signature"))
            .and_then(|s| s.as_str().map(String::from))
            .ok_or_else(|| EngineError::bad_request("bundle has no signature"))?;
        let pk = bundle["publisher"]["public_key"].as_str().unwrap_or_default().to_string();
        if !self.signer.trusted_keys().contains(&pk) {
            return Err(EngineError::forbidden("bundle is signed by an untrusted key; add it to trusted-keys/ first"));
        }
        signing::verify(&pk, &bundle, &signature).map_err(|e| EngineError::forbidden(e.to_string()))?;
        let doc: ProfileDoc =
            serde_json::from_value(bundle["profile"].clone()).map_err(|e| EngineError::bad_request(e.to_string()))?;
        let materialized = flux_core::profiles::materialize(&doc).map_err(EngineError::bad_request)?;
        let issues: Vec<_> =
            policy::validate_spec(&materialized, &self.hardware).into_iter().filter(|i| i.is_error()).collect();
        if !issues.is_empty() {
            return Err(EngineError::bad_request("imported profile does not validate")
                .with_details(json!({ "issues": issues })));
        }
        let summary = {
            let mut profiles = self.profiles.write().expect("profiles lock");
            profiles.insert(doc.clone(), ProfileSource::Imported).map_err(EngineError::bad_request)?.summary_json()
        };
        self.store.save_user_profile(&doc, true)?;
        self.audit(Record {
            kind: "profile.imported",
            actor: &who.id,
            plan_id: None,
            generation: None,
            tier: None,
            summary: format!("Imported signed profile {}", doc.id),
            details: json!({ "id": doc.id, "publisher": bundle["publisher"]["fingerprint"] }),
        });
        Ok(summary)
    }
}
