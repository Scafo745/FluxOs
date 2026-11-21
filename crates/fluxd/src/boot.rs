//! Boot sequence. Each stage is timed, logged and emitted as a `boot.stage`
//! event; the UI replays them to render the boot screen. Boot also recovers
//! from an apply that was interrupted (crash or power loss).

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use flux_core::policy::{Mode, PolicyConfig};
use flux_core::profiles::{ProfileSet, ProfileSource};
use flux_core::render::{self, RenderContext};
use flux_core::{RiskTier, SystemSpec, catalog, registry};
use serde_json::json;

use crate::audit::{AuditLog, Record};
use crate::auth::Tokens;
use crate::config::{ModeArg, ServeArgs, TargetKind};
use crate::engine::{BootReport, BootStage, Engine, EngineParts};
use crate::events::EventBus;
use crate::health;
use crate::hwprobe;
use crate::signing::Signer;
use crate::store::{ArtifactInfo, GenStatus, GenerationMeta, Store};
use crate::targets::nixos::NixosTarget;
use crate::targets::sandbox::SandboxTarget;
use crate::targets::{ApplyRequest, Target};
use crate::util::{now, read_json};

struct Booter<'a> {
    events: &'a EventBus,
    stages: Vec<BootStage>,
}

impl Booter<'_> {
    fn stage<T>(&mut self, id: &str, title: &str, f: impl FnOnce() -> Result<(T, String, bool)>) -> Result<T> {
        let t = Instant::now();
        match f() {
            Ok((v, detail, warn)) => {
                let status = if warn { "warn" } else { "ok" };
                self.push(id, title, status, detail, t);
                Ok(v)
            }
            Err(e) => {
                self.push(id, title, "fail", format!("{e:#}"), t);
                Err(e)
            }
        }
    }

    fn push(&mut self, id: &str, title: &str, status: &str, detail: String, t: Instant) {
        let s = BootStage {
            id: id.into(),
            title: title.into(),
            status: status.into(),
            detail,
            duration_ms: t.elapsed().as_millis(),
        };
        match status {
            "fail" => tracing::error!(target: "fluxd::boot", "[{}] {}: {}", s.status, s.title, s.detail),
            "warn" => tracing::warn!(target: "fluxd::boot", "[{}] {}: {}", s.status, s.title, s.detail),
            _ => tracing::info!(target: "fluxd::boot", "[{}] {}: {}", s.status, s.title, s.detail),
        }
        self.events.emit("boot.stage", serde_json::to_value(&s).unwrap_or_default());
        self.stages.push(s);
    }
}

fn adopt_spec(target: TargetKind) -> (SystemSpec, String) {
    // A NixOS image built by FluxOS ships its spec; adopt it instead of rebuilding on first boot.
    if target == TargetKind::Nixos
        && let Ok(spec) = read_json::<SystemSpec>(Path::new("/etc/fluxos/spec.json"))
    {
        return (spec, "adopted /etc/fluxos/spec.json from the installed image".into());
    }
    (SystemSpec::default(), "bootstrapped from the default profile".into())
}

pub fn boot(args: &ServeArgs) -> Result<Arc<Engine>> {
    let t0 = Instant::now();
    let started_at = now();
    let events = EventBus::new();
    let mut b = Booter { events: &events, stages: Vec::new() };
    let version = env!("CARGO_PKG_VERSION");
    let boot_id = crate::util::random_hex(8)?;
    events.emit(
        "boot.begin",
        json!({ "version": version, "target": format!("{:?}", args.target).to_lowercase(), "boot_id": boot_id }),
    );

    b.stage("core", "FluxOS core", || {
        Ok(((), format!("fluxd {version} · pid {} · {}", std::process::id(), std::env::consts::ARCH), false))
    })?;

    let store = b.stage("state", "State store", || {
        let s = Store::open(&args.state_dir)?;
        let gens = s.generation_numbers()?.len();
        Ok((s, format!("{} · {gens} generations on disk", args.state_dir.display()), false))
    })?;

    let policy = b.stage("policy", "Operator policy", || {
        let path = store.path("policy.json");
        let mut p: PolicyConfig = if path.exists() { read_json(&path)? } else { PolicyConfig::default() };
        if let Some(m) = args.mode {
            p.mode = match m {
                ModeArg::Personal => Mode::Personal,
                ModeArg::Fleet => Mode::Fleet,
            };
        }
        for w in &p.freeze_windows {
            w.validate().map_err(anyhow::Error::msg)?;
        }
        let detail = format!(
            "{:?} mode · {} change-freeze windows · {} tools disabled",
            p.mode,
            p.freeze_windows.len(),
            p.disabled_tools.len()
        );
        Ok((p, detail, false))
    })?;
    let mode = policy.mode;

    let (tokens, signer) = b.stage("secrets", "Credentials and signing key", || {
        let tokens = Tokens::load_or_create(&store.path("secrets"), mode)?;
        let signer = Signer::load_or_create(&store.path("secrets/signing.key"), &store.path("trusted-keys"))?;
        let detail = format!(
            "{} principals · {} paired screens · profile signing key {}",
            tokens.static_count(),
            tokens.session_count(),
            signer.fingerprint()
        );
        Ok(((tokens, signer), detail, false))
    })?;

    let (audit, audit_ok, audit_error) = b.stage("audit", "Audit chain", || {
        let (log, report) = AuditLog::open(&store.path("audit/audit.jsonl"))?;
        let detail = if report.ok {
            format!("{} entries verified · head {}", report.entries, &report.head[..19.min(report.head.len())])
        } else {
            format!("CHAIN BROKEN at entry {:?}: {}", report.broken_at, report.error.clone().unwrap_or_default())
        };
        Ok(((log, report.ok, report.error.clone()), detail, !report.ok))
    })?;

    b.stage("registry", "Tool registry", || {
        let blocked = registry::TOOLS.iter().filter(|t| t.blocked_reason.is_some()).count();
        let by_tier =
            |t: RiskTier| registry::TOOLS.iter().filter(|x| x.tier == t && x.blocked_reason.is_none()).count();
        Ok((
            (),
            format!(
                "{} tools allowlisted (T0 {} · T1 {} · T2 {} · T3 {}) · {blocked} blocked · default-deny",
                registry::TOOLS.len() - blocked,
                by_tier(RiskTier::Safe),
                by_tier(RiskTier::Low),
                by_tier(RiskTier::Medium),
                by_tier(RiskTier::High)
            ),
            false,
        ))
    })?;

    b.stage("catalog", "Catalogs", || {
        Ok((
            (),
            format!(
                "{} packages · {} services · {} kernels · {} schedulers · {} fonts · {} sysctl keys",
                catalog::PACKAGES.len(),
                catalog::SERVICES.len(),
                catalog::KERNELS.len(),
                catalog::SCHEDULERS.len(),
                catalog::FONTS.len(),
                catalog::SYSCTLS.len()
            ),
            false,
        ))
    })?;

    let profiles = b.stage("profiles", "Profiles", || {
        let mut set = ProfileSet::builtin().map_err(anyhow::Error::msg)?;
        let builtin = set.len();
        let mut skipped = Vec::new();
        for (doc, imported) in store.user_profile_docs()? {
            let id = doc.id.clone();
            if let Err(e) = set.insert(doc, if imported { ProfileSource::Imported } else { ProfileSource::User }) {
                skipped.push(format!("{id}: {e}"));
            }
        }
        let detail = format!(
            "{builtin} built-in ({}) · {} saved{}",
            set.iter()
                .filter(|p| p.source == ProfileSource::Builtin)
                .map(|p| p.doc.id.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            set.len() - builtin,
            if skipped.is_empty() { String::new() } else { format!(" · skipped {}", skipped.join("; ")) }
        );
        Ok((set, detail, !skipped.is_empty()))
    })?;

    let hardware = b.stage("hardware", "Hardware probe", || {
        let hw = hwprobe::probe(&args.state_dir);
        Ok((hw.clone(), format!("{} [{}]", hw.summary(), hw.source), false))
    })?;

    let target: Arc<dyn Target> = b.stage("target", "Apply target", || {
        let t: Arc<dyn Target> = match args.target {
            TargetKind::Sandbox => Arc::new(SandboxTarget::new(&args.state_dir)),
            TargetKind::Nixos => Arc::new(NixosTarget::new(
                args.nixos_config_dir.clone(),
                args.nixos_flake.clone(),
                args.nixos_dry_run,
                &args.state_dir,
            )),
        };
        let notes = t.init()?;
        let warn = notes.iter().any(|n| n.starts_with("warning"));
        let detail = format!("{} · {}", t.name(), notes.join(" · "));
        Ok((t, detail, warn))
    })?;

    b.stage("recovery", "Crash recovery", || match store.inflight()? {
        None => Ok(((), "no interrupted apply".into(), false)),
        Some(inflight) => {
            let prev_dir = store.rendered_dir(inflight.previous);
            let reboot = store.load_meta(inflight.previous).map(|m| m.requires_reboot).unwrap_or(false);
            let steps = target.restore(inflight.previous, &prev_dir, reboot)?;
            if let Ok(mut meta) = store.load_meta(inflight.generation) {
                meta.status = GenStatus::Failed;
                store.save_meta(&meta)?;
            }
            store.set_current(inflight.previous)?;
            store.clear_inflight()?;
            audit.append(Record {
                kind: "recovery.rolled_back",
                actor: "fluxd",
                plan_id: Some(&inflight.plan_id),
                generation: Some(inflight.generation),
                tier: None,
                summary: format!(
                    "Interrupted apply of generation {} rolled back to {}",
                    inflight.generation, inflight.previous
                ),
                details: json!({ "steps": steps, "started_at": inflight.started_at }),
            })?;
            Ok((
                (),
                format!(
                    "apply of generation {} was interrupted; restored generation {}",
                    inflight.generation, inflight.previous
                ),
                true,
            ))
        }
    })?;

    let current = b.stage("generation", "System generation", || {
        if let Some(n) = store.current()? {
            let meta = store.load_meta(n)?;
            return Ok((n, format!("generation {n} · {} · {:?}", meta.theme_name, meta.status), false));
        }
        let (spec, how) = adopt_spec(args.target);
        let issues: Vec<String> = flux_core::policy::validate_spec(&spec, &hardware)
            .into_iter()
            .filter(|i| i.is_error())
            .map(|i| format!("{}: {}", i.path, i.message))
            .collect();
        if !issues.is_empty() {
            anyhow::bail!("initial spec is invalid: {}", issues.join("; "));
        }
        let artifacts = render::render_all(&spec, &RenderContext { generation: 1, plan_id: None });
        let meta = GenerationMeta {
            number: 1,
            parent: None,
            created_at: now(),
            plan_id: None,
            summary: how.clone(),
            tier: RiskTier::Safe,
            status: GenStatus::Active,
            spec_digest: spec.digest(),
            actor: "fluxd".into(),
            requires_reboot: false,
            theme_name: spec.design.theme_name.clone(),
            artifacts: artifacts.iter().map(ArtifactInfo::from).collect(),
        };
        store.write_generation(&meta, &spec, &artifacts)?;
        if args.target == TargetKind::Sandbox {
            target
                .apply(&ApplyRequest {
                    generation: 1,
                    previous: 0,
                    spec: &spec,
                    artifacts: &artifacts,
                    rendered_dir: store.rendered_dir(1),
                    requires_reboot: false,
                })
                .context("materialize generation 1")?;
        }
        store.set_current(1)?;
        audit.append(Record {
            kind: "generation.bootstrap",
            actor: "fluxd",
            plan_id: None,
            generation: Some(1),
            tier: Some(0),
            summary: format!("Generation 1 {how}"),
            details: json!({ "spec_digest": spec.digest(), "target": target.name() }),
        })?;
        Ok((1, format!("generation 1 {how}"), false))
    })?;

    let mut degraded: Vec<String> = Vec::new();
    if !audit_ok {
        degraded.push(format!("audit chain broken: {}", audit_error.unwrap_or_default()));
    }

    b.stage("health", "Health checks", || {
        let spec = store.load_spec(current)?;
        let meta = store.load_meta(current)?;
        let mut checks = target.health(&spec, current, &meta.artifacts);
        let mut repaired = false;
        if health::any_failed(&checks) && args.target == TargetKind::Sandbox {
            // Drifted or damaged live tree: re-materialize the committed generation.
            target.restore(current, &store.rendered_dir(current), false)?;
            checks = target.health(&spec, current, &meta.artifacts);
            repaired = true;
        }
        let passed = checks.iter().filter(|c| c.status == health::HealthStatus::Pass).count();
        let failed: Vec<String> =
            checks.iter().filter(|c| c.status == health::HealthStatus::Fail).map(|c| c.name.clone()).collect();
        if !failed.is_empty() {
            degraded.push(format!("boot health failed: {}", failed.join(", ")));
        }
        Ok((
            (),
            format!(
                "{passed}/{} passed{}{}",
                checks.len(),
                if repaired { " · drift repaired" } else { "" },
                if failed.is_empty() { String::new() } else { format!(" · failing: {}", failed.join(", ")) }
            ),
            repaired || !failed.is_empty(),
        ))
    })?;

    let stages = b.stages;
    let engine = Engine::new(EngineParts {
        mode,
        policy,
        dev: args.dev,
        store,
        audit,
        events,
        tokens,
        signer,
        hardware,
        target,
        profiles,
        current,
        boot_id,
        agent_url: args.agent_url.clone(),
        allowed_hosts: args
            .allowed_hosts
            .iter()
            .map(|h| h.trim().to_ascii_lowercase())
            .filter(|h| !h.is_empty())
            .collect(),
    });
    for d in degraded {
        engine.set_degraded(d);
    }
    *engine.boot.write().expect("boot lock") = BootReport {
        version: version.into(),
        boot_id: engine.boot_id.clone(),
        started_at,
        ready: false,
        duration_ms: t0.elapsed().as_millis(),
        target: engine.target.name().into(),
        generation: current,
        stages,
    };
    Ok(Arc::new(engine))
}

/// Marks boot complete once the API is listening.
pub fn mark_ready(engine: &Engine, listen: &str, ui: bool) {
    let t = Instant::now();
    let stage = BootStage {
        id: "api".into(),
        title: "API".into(),
        status: "ok".into(),
        detail: format!(
            "listening on http://{listen} · SSE /v1/events · MCP /mcp{}",
            if ui { " · web UI /" } else { "" }
        ),
        duration_ms: t.elapsed().as_millis(),
    };
    engine.events.emit("boot.stage", serde_json::to_value(&stage).unwrap_or_default());
    let report = {
        let mut boot = engine.boot.write().expect("boot lock");
        boot.stages.push(stage);
        boot.ready = true;
        boot.clone()
    };
    engine.events.emit(
        "boot.ready",
        json!({
            "generation": report.generation,
            "duration_ms": report.duration_ms,
            "target": report.target,
            "boot_id": engine.boot_id,
            "degraded": engine.degraded_reasons(),
        }),
    );
    // The code goes to the log and secrets/pairing.code, never to the API.
    tracing::info!(
        target: "fluxd::pairing",
        "pair the FluxOS UI with code {} (also in {}/secrets/pairing.code)",
        engine.tokens.pairing_code(),
        engine.store.path("").display().to_string().trim_end_matches('/')
    );
    let _ = engine.audit.append(Record {
        kind: "system.boot",
        actor: "fluxd",
        plan_id: None,
        generation: Some(report.generation),
        tier: None,
        summary: format!("fluxd {} booted in {} ms on {}", report.version, report.duration_ms, report.target),
        details: json!({ "degraded": engine.degraded_reasons() }),
    });
}
