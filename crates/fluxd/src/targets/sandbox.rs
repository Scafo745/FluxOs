//! Sandbox target: materializes each generation as a real file tree under
//! `<state>/sandbox/root`, with simulated boot entries that keep every
//! previous generation selectable. Used for development, demos, CI and as
//! the preview stage for real targets.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use flux_core::SystemSpec;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{ApplyOutcome, ApplyRequest, Target};
use crate::health::{self, HealthResult};
use crate::store::ArtifactInfo;
use crate::util::write_atomic;

pub struct SandboxTarget {
    base: PathBuf,
}

fn copy_tree(from: &Path, to: &Path) -> Result<usize> {
    let mut n = 0;
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from).with_context(|| format!("read {}", from.display()))? {
        let e = e?;
        let ty = e.file_type()?;
        let dest = to.join(e.file_name());
        if ty.is_dir() {
            n += copy_tree(&e.path(), &dest)?;
        } else if ty.is_file() {
            fs::copy(e.path(), &dest)?;
            n += 1;
        }
    }
    Ok(n)
}

impl SandboxTarget {
    pub fn new(state: &Path) -> Self {
        Self { base: state.join("sandbox") }
    }

    pub fn root(&self) -> PathBuf {
        self.base.join("root")
    }

    fn entries(&self) -> PathBuf {
        self.base.join("boot").join("entries")
    }

    /// Swaps the live root for a copy of `rendered` in one rename.
    fn materialize(&self, rendered: &Path) -> Result<usize> {
        let next = self.base.join("root.next");
        let prev = self.base.join("root.prev");
        for d in [&next, &prev] {
            if d.exists() {
                fs::remove_dir_all(d)?;
            }
        }
        let n = copy_tree(rendered, &next)?;
        let root = self.root();
        if root.exists() {
            fs::rename(&root, &prev)?;
        }
        fs::rename(&next, &root)?;
        if prev.exists() {
            fs::remove_dir_all(&prev)?;
        }
        Ok(n)
    }

    fn write_boot_entry(&self, generation: u64, spec: &SystemSpec, default: bool) -> Result<()> {
        let kernel = flux_core::catalog::kernel(spec.kernel.variant);
        let entry = format!(
            "title FluxOS generation {generation} ({})\nversion {}\nlinux /nix/store/simulated-{}/bzImage\noptions init=/nix/var/nix/profiles/system-{generation}-link/init {}\n",
            spec.design.theme_name,
            kernel.version_hint,
            kernel.nix_packages,
            kernel.extra_params.join(" ")
        );
        write_atomic(&self.entries().join(format!("fluxos-gen-{generation}.conf")), entry.as_bytes())?;
        if default {
            write_atomic(
                &self.base.join("boot").join("loader.conf"),
                format!("default fluxos-gen-{generation}.conf\ntimeout {}\n", spec.boot.loader_timeout_s).as_bytes(),
            )?;
        }
        Ok(())
    }

    fn boot_default(&self) -> Option<String> {
        fs::read_to_string(self.base.join("boot").join("loader.conf"))
            .ok()?
            .lines()
            .find_map(|l| l.strip_prefix("default ").map(|s| s.trim().to_string()))
    }
}

impl Target for SandboxTarget {
    fn name(&self) -> &'static str {
        "sandbox"
    }

    fn describe(&self) -> Value {
        json!({
            "kind": "sandbox",
            "root": self.root(),
            "boot_entries": self.entries(),
            "note": "Materializes generations as files; no host changes.",
        })
    }

    fn init(&self) -> Result<Vec<String>> {
        fs::create_dir_all(self.entries())?;
        let entries = fs::read_dir(self.entries())?.count();
        Ok(vec![format!("sandbox root {} ({entries} boot entries)", self.root().display())])
    }

    fn apply(&self, req: &ApplyRequest<'_>) -> Result<ApplyOutcome> {
        let mut steps = vec![format!("render {} artifacts for generation {}", req.artifacts.len(), req.generation)];
        let files = self.materialize(&req.rendered_dir)?;
        steps.push(format!("materialize {files} files into {}", self.root().display()));
        self.write_boot_entry(req.generation, req.spec, true)?;
        steps.push(format!(
            "boot entry fluxos-gen-{} set as default; fluxos-gen-{} and older stay bootable",
            req.generation, req.previous
        ));
        let message = if req.requires_reboot {
            steps.push("kernel/boot change staged: takes effect after reboot".into());
            format!("Generation {} staged for the next boot", req.generation)
        } else {
            format!("Generation {} is live", req.generation)
        };
        Ok(ApplyOutcome { staged: req.requires_reboot, steps, message })
    }

    fn restore(&self, generation: u64, rendered_dir: &Path, _requires_reboot: bool) -> Result<Vec<String>> {
        let files = self.materialize(rendered_dir)?;
        let spec: SystemSpec = serde_json::from_str(&fs::read_to_string(rendered_dir.join("etc/fluxos/spec.json"))?)?;
        self.write_boot_entry(generation, &spec, true)?;
        Ok(vec![format!("restored generation {generation} ({files} files) and made it the default boot entry")])
    }

    fn health(&self, spec: &SystemSpec, generation: u64, artifacts: &[ArtifactInfo]) -> Vec<HealthResult> {
        let root = self.root();
        let mut out = Vec::new();
        let mut mismatched = Vec::new();
        let mut loaded = Vec::new();
        for a in artifacts {
            match fs::read(root.join(&a.path)) {
                Ok(bytes) => {
                    if hex::encode(Sha256::digest(&bytes)) != a.sha256 {
                        mismatched.push(a.path.clone());
                    }
                    loaded
                        .push(flux_core::render::Artifact::new(&a.path, String::from_utf8_lossy(&bytes).into_owned()));
                }
                Err(_) => mismatched.push(format!("{} (missing)", a.path)),
            }
        }
        out.push(if mismatched.is_empty() {
            HealthResult::pass(
                "system.integrity",
                "Live files match generation",
                format!("{} files verified by SHA-256", artifacts.len()),
            )
        } else {
            HealthResult::fail(
                "system.integrity",
                "Live files match generation",
                format!("mismatch: {}", mismatched.join(", ")),
            )
        });
        out.extend(health::artifact_checks(spec, &loaded));
        let expected = format!("fluxos-gen-{generation}.conf");
        out.push(match self.boot_default() {
            Some(d) if d == expected && self.entries().join(&expected).exists() => {
                let n = fs::read_dir(self.entries()).map(|r| r.count()).unwrap_or(0);
                HealthResult::pass("boot.entries", "Boot entries", format!("default {d}; {n} generations bootable"))
            }
            other => HealthResult::fail(
                "boot.entries",
                "Boot entries",
                format!("default entry is {other:?}, expected {expected}"),
            ),
        });
        out
    }

    fn reboot(&self) -> Result<String> {
        Ok("simulated reboot: staged generation is now running".into())
    }
}
