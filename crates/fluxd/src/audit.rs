//! Append-only, hash-chained audit log (JSON lines).
//!
//! Each entry stores the SHA-256 of the previous entry, so editing or
//! deleting any line breaks verification from that point on. There is no
//! API to remove entries. Every append is also emitted on the `fluxd::audit`
//! tracing target, which journald/syslog collect when running under systemd.

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::util::now;

pub const GENESIS: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub seq: u64,
    pub ts: String,
    pub kind: String,
    pub actor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<u8>,
    pub summary: String,
    #[serde(default)]
    pub details: Value,
    pub prev_hash: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct VerifyReport {
    pub ok: bool,
    pub entries: u64,
    pub head: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub broken_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn entry_hash(entry: &AuditEntry) -> String {
    let mut unsigned = serde_json::to_value(entry).unwrap_or_default();
    if let Some(obj) = unsigned.as_object_mut() {
        obj.remove("hash");
    }
    let body = serde_json::to_vec(&unsigned).unwrap_or_default();
    let mut h = Sha256::new();
    h.update(entry.prev_hash.as_bytes());
    h.update(b"\n");
    h.update(&body);
    format!("sha256:{}", hex::encode(h.finalize()))
}

pub struct AuditLog {
    path: PathBuf,
    head: Mutex<(u64, String)>,
}

/// Fields for a new entry.
pub struct Record<'a> {
    pub kind: &'a str,
    pub actor: &'a str,
    pub plan_id: Option<&'a str>,
    pub generation: Option<u64>,
    pub tier: Option<u8>,
    pub summary: String,
    pub details: Value,
}

impl AuditLog {
    /// Opens (or creates) the log and verifies the whole chain.
    pub fn open(path: &Path) -> Result<(Self, VerifyReport)> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        if !path.exists() {
            fs::File::create(path).with_context(|| format!("create {}", path.display()))?;
        }
        let report = verify(path)?;
        let head = (report.entries, report.head.clone());
        Ok((Self { path: path.to_path_buf(), head: Mutex::new(head) }, report))
    }

    pub fn append(&self, r: Record<'_>) -> Result<AuditEntry> {
        let mut head = self.head.lock().expect("audit lock");
        let mut entry = AuditEntry {
            seq: head.0 + 1,
            ts: now(),
            kind: r.kind.to_string(),
            actor: r.actor.to_string(),
            plan_id: r.plan_id.map(String::from),
            generation: r.generation,
            tier: r.tier,
            summary: r.summary,
            details: r.details,
            prev_hash: head.1.clone(),
            hash: String::new(),
        };
        entry.hash = entry_hash(&entry);
        let mut line = serde_json::to_vec(&entry)?;
        line.push(b'\n');
        let mut f = OpenOptions::new()
            .append(true)
            .open(&self.path)
            .with_context(|| format!("open {}", self.path.display()))?;
        f.write_all(&line)?;
        f.sync_data()?;
        *head = (entry.seq, entry.hash.clone());
        tracing::info!(target: "fluxd::audit", seq = entry.seq, kind = %entry.kind, actor = %entry.actor, "{}", entry.summary);
        Ok(entry)
    }

    pub fn head(&self) -> (u64, String) {
        self.head.lock().expect("audit lock").clone()
    }

    /// Newest-first page of entries.
    pub fn page(&self, before_seq: Option<u64>, limit: usize, kind_prefix: Option<&str>) -> Result<Vec<AuditEntry>> {
        let f = fs::File::open(&self.path)?;
        let mut out: Vec<AuditEntry> = Vec::new();
        for line in BufReader::new(f).lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let Ok(e) = serde_json::from_str::<AuditEntry>(&line) else { continue };
            if before_seq.is_some_and(|b| e.seq >= b) {
                continue;
            }
            if kind_prefix.is_some_and(|k| !e.kind.starts_with(k)) {
                continue;
            }
            out.push(e);
        }
        out.reverse();
        out.truncate(limit);
        Ok(out)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub fn verify(path: &Path) -> Result<VerifyReport> {
    let f = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut prev = GENESIS.to_string();
    let mut expected_seq = 1u64;
    for (i, line) in BufReader::new(f).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let fail = |msg: String| VerifyReport {
            ok: false,
            entries: expected_seq - 1,
            head: prev.clone(),
            broken_at: Some(expected_seq),
            error: Some(msg),
        };
        let entry: AuditEntry = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(e) => return Ok(fail(format!("line {}: unparsable entry: {e}", i + 1))),
        };
        if entry.seq != expected_seq {
            return Ok(fail(format!("line {}: expected seq {expected_seq}, found {}", i + 1, entry.seq)));
        }
        if entry.prev_hash != prev {
            return Ok(fail(format!("seq {}: prev_hash does not match the previous entry", entry.seq)));
        }
        if entry_hash(&entry) != entry.hash {
            return Ok(fail(format!("seq {}: content hash mismatch (entry was modified)", entry.seq)));
        }
        prev = entry.hash;
        expected_seq += 1;
    }
    Ok(VerifyReport { ok: true, entries: expected_seq - 1, head: prev, broken_at: None, error: None })
}
