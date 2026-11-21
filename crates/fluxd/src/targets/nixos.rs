//! NixOS target.
//!
//! Apply flow for generation N:
//! 1. write `<config_dir>/generated.nix` atomically (the host configuration
//!    imports it; see `nix/module.nix`),
//! 2. `nixos-rebuild build` as the sandbox test (nothing is activated),
//! 3. `nixos-rebuild switch`, or `nixos-rebuild boot` for kernel and boot
//!    changes so the running system is untouched until reboot,
//! 4. on any failure, restore the previous file and configuration.
//!
//! NixOS keeps every previous system generation in the boot menu, which is
//! the fallback if a new kernel does not boot.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use flux_core::SystemSpec;
use flux_core::catalog;
use flux_core::spec::SessionMode;
use serde_json::{Value, json};

use super::{ApplyOutcome, ApplyRequest, Target};
use crate::health::HealthResult;
use crate::store::ArtifactInfo;
use crate::util::write_atomic;

const MODULE: &str = "etc/nixos/fluxos/generated.nix";
const REBUILD_TIMEOUT: Duration = Duration::from_secs(45 * 60);

pub struct NixosTarget {
    config_dir: PathBuf,
    flake: Option<String>,
    dry_run: bool,
    log_dir: PathBuf,
}

impl NixosTarget {
    pub fn new(config_dir: PathBuf, flake: Option<String>, dry_run: bool, state: &Path) -> Self {
        Self { config_dir, flake, dry_run, log_dir: state.join("logs") }
    }

    fn module_path(&self) -> PathBuf {
        self.config_dir.join("generated.nix")
    }

    /// Runs a command with output captured to a log file and a hard timeout.
    fn run(&self, label: &str, program: &str, args: &[&str], timeout: Duration) -> Result<String> {
        let shown = format!("{program} {}", args.join(" "));
        if self.dry_run {
            tracing::info!(target: "fluxd::nixos", "[dry-run] {shown}");
            return Ok(format!("[dry-run] {shown}"));
        }
        fs::create_dir_all(&self.log_dir)?;
        let log_path = self.log_dir.join(format!("{label}.log"));
        let log = fs::File::create(&log_path)?;
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()
            .with_context(|| format!("spawn {shown}"))?;
        let start = Instant::now();
        loop {
            if let Some(status) = child.try_wait()? {
                if status.success() {
                    return Ok(format!("{shown} (ok, {}s)", start.elapsed().as_secs()));
                }
                let tail = fs::read_to_string(&log_path).unwrap_or_default();
                let tail: Vec<&str> = tail.lines().rev().take(15).collect();
                let tail: Vec<&str> = tail.into_iter().rev().collect();
                bail!("{shown} failed ({status}). Last output:\n{}", tail.join("\n"));
            }
            if start.elapsed() > timeout {
                let _ = child.kill();
                bail!("{shown} timed out after {}s", timeout.as_secs());
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    fn rebuild(&self, action: &str, label: &str) -> Result<String> {
        let mut args = vec![action];
        if let Some(f) = &self.flake {
            args.push("--flake");
            args.push(f);
        }
        self.run(label, "nixos-rebuild", &args, REBUILD_TIMEOUT)
    }

    fn write_module(&self, content: &str) -> Result<()> {
        if self.dry_run {
            let p = self.log_dir.join("dry-run-generated.nix");
            return write_atomic(&p, content.as_bytes());
        }
        write_atomic(&self.module_path(), content.as_bytes())
    }

    fn systemctl(args: &[&str]) -> (bool, String) {
        match Command::new("systemctl").args(args).stdin(Stdio::null()).output() {
            Ok(o) => (o.status.success(), String::from_utf8_lossy(&o.stdout).trim().to_string()),
            Err(e) => (false, e.to_string()),
        }
    }
}

impl Target for NixosTarget {
    fn name(&self) -> &'static str {
        "nixos"
    }

    fn describe(&self) -> Value {
        json!({
            "kind": "nixos",
            "module": self.module_path(),
            "flake": self.flake,
            "dry_run": self.dry_run,
            "note": "Writes generated.nix and runs nixos-rebuild; previous generations stay in the boot menu.",
        })
    }

    fn init(&self) -> Result<Vec<String>> {
        let mut notes = Vec::new();
        if self.dry_run {
            notes.push("dry-run: nixos-rebuild commands are logged, not executed".into());
            return Ok(notes);
        }
        if !Path::new("/etc/NIXOS").exists() {
            bail!("this host is not NixOS (/etc/NIXOS missing); use --target sandbox or --nixos-dry-run");
        }
        fs::create_dir_all(&self.config_dir).with_context(|| format!("create {}", self.config_dir.display()))?;
        notes.push(format!("managing {}", self.module_path().display()));
        let host_cfg = Path::new("/etc/nixos/configuration.nix");
        if self.flake.is_none()
            && let Ok(src) = fs::read_to_string(host_cfg)
            && !src.contains("fluxos")
        {
            notes.push("warning: /etc/nixos/configuration.nix does not reference fluxos; import ./fluxos/generated.nix or the fluxos NixOS module".into());
        }
        Ok(notes)
    }

    fn apply(&self, req: &ApplyRequest<'_>) -> Result<ApplyOutcome> {
        let module = req.artifacts.iter().find(|a| a.path == MODULE).context("generated.nix was not rendered")?;
        let previous = fs::read_to_string(self.module_path()).ok();
        let mut steps = Vec::new();

        self.write_module(&module.content)?;
        steps.push(format!("wrote {}", self.module_path().display()));

        let restore_file = |steps: &mut Vec<String>| {
            if let Some(prev) = &previous {
                let _ = self.write_module(prev);
                steps.push("restored previous generated.nix".into());
            }
        };

        // Sandbox test: build the full system closure without activating it.
        match self.rebuild("build", &format!("gen-{}-build", req.generation)) {
            Ok(s) => steps.push(s),
            Err(e) => {
                restore_file(&mut steps);
                return Err(e.context("nixos-rebuild build failed; nothing was activated"));
            }
        }

        let action = if req.requires_reboot { "boot" } else { "switch" };
        match self.rebuild(action, &format!("gen-{}-{action}", req.generation)) {
            Ok(s) => steps.push(s),
            Err(e) => {
                restore_file(&mut steps);
                if action == "switch" {
                    let _ = self.rebuild("switch", &format!("gen-{}-revert", req.generation));
                    steps.push("re-activated previous configuration".into());
                }
                return Err(e.context(format!("nixos-rebuild {action} failed")));
            }
        }
        let message = if req.requires_reboot {
            format!(
                "Generation {} installed as the default boot entry; reboot to activate. The previous entry stays in the boot menu.",
                req.generation
            )
        } else {
            format!("Generation {} is live", req.generation)
        };
        Ok(ApplyOutcome { staged: req.requires_reboot, steps, message })
    }

    fn restore(&self, generation: u64, rendered_dir: &Path, requires_reboot: bool) -> Result<Vec<String>> {
        let content = fs::read_to_string(rendered_dir.join(MODULE)).context("stored generated.nix missing")?;
        self.write_module(&content)?;
        let action = if requires_reboot { "boot" } else { "switch" };
        let s = self.rebuild(action, &format!("restore-{generation}"))?;
        Ok(vec![format!("restored generation {generation}"), s])
    }

    fn health(&self, spec: &SystemSpec, _generation: u64, _artifacts: &[ArtifactInfo]) -> Vec<HealthResult> {
        if self.dry_run {
            return vec![HealthResult::warn(
                "nixos.dry_run",
                "NixOS dry-run",
                "commands were not executed; system checks skipped",
            )];
        }
        let mut out = Vec::new();
        let (_, state) = Self::systemctl(&["is-system-running"]);
        out.push(match state.as_str() {
            "running" => HealthResult::pass("system.state", "systemd state", "running"),
            "degraded" => {
                let (_, failed) = Self::systemctl(&["--failed", "--no-legend", "--plain"]);
                HealthResult::warn("system.state", "systemd state", format!("degraded: {}", failed.replace('\n', "; ")))
            }
            other => HealthResult::fail("system.state", "systemd state", other.to_string()),
        });
        let route = Command::new("ip")
            .args(["route", "show", "default"])
            .output()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false);
        out.push(if route {
            HealthResult::pass("network.route", "Network", "default route present")
        } else {
            HealthResult::warn("network.route", "Network", "no default route")
        });
        // Every FluxOS session is graphical: a display manager, or cage in kiosk/exam mode.
        let unit =
            if spec.session.mode == SessionMode::Standard { "display-manager.service" } else { "cage-tty1.service" };
        let (ok, s) = Self::systemctl(&["is-active", unit]);
        out.push(if ok {
            HealthResult::pass("display.manager", "Display", format!("{unit} {s}"))
        } else {
            HealthResult::fail("display.manager", "Display", format!("{unit} is {s}"))
        });
        for id in &spec.services.enabled {
            if let Some(svc) = catalog::service(id) {
                for unit in svc.units {
                    let (ok, s) = Self::systemctl(&["is-active", unit]);
                    out.push(if ok {
                        HealthResult::pass(&format!("service.{id}"), svc.name, format!("{unit} {s}"))
                    } else {
                        HealthResult::fail(&format!("service.{id}"), svc.name, format!("{unit} is {s}"))
                    });
                }
            }
        }
        out
    }

    fn reboot(&self) -> Result<String> {
        if self.dry_run {
            return Ok("[dry-run] systemctl reboot".into());
        }
        let (ok, s) = Self::systemctl(&["reboot"]);
        if !ok {
            bail!("systemctl reboot failed: {s}");
        }
        Ok("rebooting".into())
    }
}
