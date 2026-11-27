//! fluxd: the FluxOS system daemon.
//!
//! The daemon is the only component that changes the machine. It accepts
//! typed plans (from the AI orchestrator, the web UI, MCP clients or the
//! CLI), validates them with the policy engine, snapshots, applies through
//! a target adapter, verifies health, and commits or rolls back. Every step
//! lands in a hash-chained audit log.

mod api;
mod audit;
mod auth;
mod boot;
mod config;
mod engine;
mod events;
mod health;
mod hwprobe;
mod mcp;
mod session;
mod signing;
mod store;
mod targets;
mod util;

use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use flux_core::profiles::ProfileSet;
use flux_core::render::{RenderContext, render_all};
use flux_core::{SystemSpec, registry};
use serde_json::json;
use tracing_subscriber::EnvFilter;

use crate::config::{Cli, Command, ServeArgs};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,tower_http=warn")))
        .with_target(true)
        .init();

    match Cli::parse().command {
        Command::Serve(args) => serve(args).await,
        Command::AuditVerify { state_dir } => {
            let report = audit::verify(&state_dir.join("audit/audit.jsonl"))?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if !report.ok {
                std::process::exit(2);
            }
            Ok(())
        }
        Command::Render { profile, out, generation } => {
            let set = ProfileSet::builtin().map_err(anyhow::Error::msg)?;
            let p = set.get(&profile).with_context(|| format!("unknown profile {profile}"))?;
            for a in render_all(&p.spec, &RenderContext { generation, plan_id: None }) {
                let path = out.join(&a.path);
                util::write_atomic(&path, a.content.as_bytes())?;
                println!("{:>8}  {}", a.bytes, path.display());
            }
            Ok(())
        }
        Command::PairCode { state_dir } => {
            let path = state_dir.join("secrets/pairing.code");
            let code = std::fs::read_to_string(&path)
                .with_context(|| format!("read {} (is fluxd running with this state dir?)", path.display()))?;
            println!("{}", code.trim());
            eprintln!("Enter it in the FluxOS UI, or open /#pair={} on the UI address.", code.trim().replace('-', ""));
            Ok(())
        }
        Command::Schema => {
            let v = json!({ "plan_request": api::plan_request_schema(), "spec": registry::schema_of::<SystemSpec>() });
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(())
        }
        Command::Tools => {
            println!("{}", serde_json::to_string_pretty(&registry::infos())?);
            Ok(())
        }
    }
}

async fn serve(args: ServeArgs) -> Result<()> {
    if let Some(dir) = &args.ui_dir
        && !dir.join("index.html").is_file()
    {
        anyhow::bail!("--ui-dir {} has no index.html; build the UI first (cd ui && npm run build)", dir.display());
    }
    let boot_args = args.clone();
    let engine = tokio::task::spawn_blocking(move || boot::boot(&boot_args)).await??;
    let app = api::router(engine.clone(), &args.cors_origins, args.ui_dir.as_deref());
    let listener = tokio::net::TcpListener::bind(args.listen).await.with_context(|| format!("bind {}", args.listen))?;
    boot::mark_ready(&engine, &args.listen.to_string(), args.ui_dir.is_some());

    let (tx, rx) = tokio::sync::watch::channel(false);
    tokio::spawn(async move {
        shutdown_signal().await;
        let _ = tx.send(true);
    });
    let mut graceful = rx.clone();
    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        let _ = graceful.changed().await;
    });
    let mut forced = rx;
    tokio::select! {
        r = server => r?,
        _ = async move {
            let _ = forced.changed().await;
            // Long-lived SSE streams would hold graceful shutdown open.
            tokio::time::sleep(Duration::from_secs(3)).await;
        } => tracing::info!("closing open streams"),
    }
    let _ = engine.audit.append(audit::Record {
        kind: "system.shutdown",
        actor: "fluxd",
        plan_id: None,
        generation: Some(engine.current_generation()),
        tier: None,
        summary: "fluxd stopped".into(),
        details: json!({ "uptime_s": engine.started.elapsed().as_secs() }),
    });
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = term => {},
    }
    tracing::info!("shutdown requested");
}
