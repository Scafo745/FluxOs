//! NixOS target.
//!
//! A generation has two parts: `generated.nix`, which the host configuration
//! imports (see `nix/module.nix`), and live files (theme, tokens, wallpaper,
//! desktop config) that the module links from `/run/fluxos/current`.
//!
//! Apply flow for generation N:
//! 1. if its `generated.nix` equals the current one apart from comments and
//!    the boot-menu tag, only live files changed: publish them and stop,
//! 2. otherwise write `<config_dir>/generated.nix` atomically,
//! 3. `nixos-rebuild build` as the sandbox test (nothing is activated),
//! 4. `nixos-rebuild switch`, or `nixos-rebuild boot` for kernel and boot
//!    changes so the running system is untouched until reboot,
//! 5. publish the live files,
//! 6. on any failure, restore the previous file and configuration.
//!
//! NixOS keeps every previous system generation in the boot menu, which is
//! the fallback if a new kernel does not boot.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use flux_core::SystemSpec;
use flux_core::catalog;
use flux_core::render::nix::normalize_module;
use flux_core::render::{LIVE_DIR, LIVE_PREFIXES, live_path};
use flux_core::spec::{DesktopEnv, SessionMode};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{ApplyOutcome, ApplyRequest, Target};
use crate::health::HealthResult;
use crate::store::ArtifactInfo;
use crate::util::write_atomic;

const MODULE: &str = "etc/nixos/fluxos/generated.nix";
const REBUILD_TIMEOUT: Duration = Duration::from_secs(45 * 60);
/// How long a freshly switched session unit may take to become active.
const SESSION_GRACE: Duration = Duration::from_secs(30);

/// Copies a rendered tree, making it world-readable (fluxd runs with umask 077).
fn copy_readable(from: &Path, to: &Path) -> Result<usize> {
    let mut n = 0;
    fs::create_dir_all(to)?;
    fs::set_permissions(to, fs::Permissions::from_mode(0o755))?;
    for e in fs::read_dir(from).with_context(|| format!("read {}", from.display()))? {
        let e = e?;
        let ty = e.file_type()?;
        let dest = to.join(e.file_name());
        if ty.is_dir() {
            n += copy_readable(&e.path(), &dest)?;
        } else if ty.is_file() {
            fs::copy(e.path(), &dest)?;
            fs::set_permissions(&dest, fs::Permissions::from_mode(0o644))?;
            n += 1;
        }
    }
    Ok(n)
}

/// Whether two modules differ in anything but comments and the generation tag.
fn needs_rebuild(current: Option<&str>, next: &str) -> bool {
    current.is_none_or(|c| normalize_module(c) != normalize_module(next))
}

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

    /// Where the module is written (a log file in dry-run mode).
    fn written_module_path(&self) -> PathBuf {
        if self.dry_run { self.log_dir.join("dry-run-generated.nix") } else { self.module_path() }
    }

    fn write_module(&self, content: &str) -> Result<()> {
        write_atomic(&self.written_module_path(), content.as_bytes())
    }

    fn current_module(&self) -> Option<String> {
        fs::read_to_string(self.written_module_path()).ok()
    }

    /// `/run/fluxos` (under the log dir in dry-run mode, so tests need no root).
    fn live_root(&self) -> PathBuf {
        match LIVE_DIR.rsplit_once('/') {
            Some((root, _)) if !self.dry_run => PathBuf::from(root),
            _ => self.log_dir.join("live"),
        }
    }

    /// Copies a generation's live files to `<root>/gen-N`, then repoints
    /// `<root>/current` at them in one rename. Keeps the previous tree.
    fn publish_live(&self, generation: u64, rendered_dir: &Path) -> Result<usize> {
        let root = self.live_root();
        fs::create_dir_all(&root)?;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755))?;
        let name = format!("gen-{generation}");
        let staging = root.join(format!(".{name}.tmp"));
        if staging.exists() {
            fs::remove_dir_all(&staging)?;
        }
        fs::create_dir_all(&staging)?;
        fs::set_permissions(&staging, fs::Permissions::from_mode(0o755))?;
        let mut files = 0;
        for (from, to) in LIVE_PREFIXES {
            let src = rendered_dir.join(from.trim_end_matches('/'));
            if src.is_dir() {
                files += copy_readable(&src, &staging.join(to.trim_end_matches('/')))?;
            }
        }
        let dir = root.join(&name);
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
        fs::rename(&staging, &dir)?;

        let current = root.join("current");
        let previous = fs::read_link(&current).ok().and_then(|p| p.to_str().map(String::from));
        let link_tmp = root.join(".current.tmp");
        let _ = fs::remove_file(&link_tmp);
        std::os::unix::fs::symlink(&name, &link_tmp)?;
        fs::rename(&link_tmp, &current)?;

        for e in fs::read_dir(&root)?.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.starts_with("gen-") && n != name && Some(&n) != previous.as_ref() {
                let _ = fs::remove_dir_all(e.path());
            }
        }
        Ok(files)
    }

    /// Waits for a unit that a switch may still be starting.
    fn wait_active(unit: &str, grace: Duration) -> (bool, String) {
        let start = Instant::now();
        loop {
            let (ok, state) = Self::systemctl(&["is-active", unit]);
            if ok || start.elapsed() >= grace || !matches!(state.as_str(), "activating" | "inactive" | "reloading") {
                return (ok, state);
            }
            std::thread::sleep(Duration::from_millis(500));
        }
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
            "live": self.live_root().join("current"),
            "flake": self.flake,
            "dry_run": self.dry_run,
            "note": "Writes generated.nix and runs nixos-rebuild when the configuration changes; theme and layout changes only swap live files. Previous generations stay in the boot menu.",
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
        let previous = self.current_module();
        let mut steps = Vec::new();

        if !needs_rebuild(previous.as_deref(), &module.content) {
            let files = self.publish_live(req.generation, &req.rendered_dir)?;
            steps.push(format!(
                "the NixOS configuration is unchanged; {LIVE_DIR} now serves generation {} ({files} live files)",
                req.generation
            ));
            return Ok(ApplyOutcome {
                staged: false,
                steps,
                message: format!("Generation {} is live (no rebuild needed)", req.generation),
            });
        }

        self.write_module(&module.content)?;
        steps.push(format!("wrote {}", self.written_module_path().display()));

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
        // Theme changes show at once, even when kernel or boot changes wait for a reboot.
        let files = self.publish_live(req.generation, &req.rendered_dir)?;
        steps.push(format!("{LIVE_DIR} now serves generation {} ({files} live files)", req.generation));
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
        let mut steps = vec![format!("restored generation {generation}")];
        if needs_rebuild(self.current_module().as_deref(), &content) {
            self.write_module(&content)?;
            let action = if requires_reboot { "boot" } else { "switch" };
            steps.push(self.rebuild(action, &format!("restore-{generation}"))?);
        } else {
            steps.push("NixOS configuration unchanged; no rebuild needed".into());
        }
        let files = self.publish_live(generation, rendered_dir)?;
        steps.push(format!("{LIVE_DIR} now serves generation {generation} ({files} live files)"));
        Ok(steps)
    }

    fn on_boot(&self, generation: u64, rendered_dir: &Path) -> Result<Option<String>> {
        let files = self.publish_live(generation, rendered_dir)?;
        Ok(Some(format!("{LIVE_DIR} serves generation {generation} ({files} live files)")))
    }

    fn health(&self, spec: &SystemSpec, generation: u64, artifacts: &[ArtifactInfo]) -> Vec<HealthResult> {
        let mut out = Vec::new();
        let current = self.live_root().join("current");
        let mut bad = Vec::new();
        let mut checked = 0;
        for a in artifacts {
            let Some(rel) = live_path(&a.path) else { continue };
            checked += 1;
            match fs::read(current.join(&rel)) {
                Ok(bytes) if hex::encode(Sha256::digest(&bytes)) == a.sha256 => {}
                Ok(_) => bad.push(rel),
                Err(_) => bad.push(format!("{rel} (missing)")),
            }
        }
        out.push(if bad.is_empty() {
            HealthResult::pass(
                "live.files",
                "Live theme files",
                format!("generation {generation}: {checked} files verified by SHA-256"),
            )
        } else {
            HealthResult::fail("live.files", "Live theme files", format!("mismatch: {}", bad.join(", ")))
        });
        if self.dry_run {
            out.push(HealthResult::warn(
                "nixos.dry_run",
                "NixOS dry-run",
                "commands were not executed; system checks skipped",
            ));
            return out;
        }
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
        // Every FluxOS session is graphical: cage for the FluxOS shell and kiosk/exam mode,
        // a display manager for the other desktops.
        let cage = spec.session.mode != SessionMode::Standard || spec.desktop.environment == DesktopEnv::Fluxos;
        let unit = if cage { "cage-tty1.service" } else { "display-manager.service" };
        let (ok, s) = Self::wait_active(unit, SESSION_GRACE);
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
