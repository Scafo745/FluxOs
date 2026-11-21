//! Principals, roles, bearer tokens and screen pairing.
//!
//! Static tokens are generated at first boot into `secrets/` (mode 0600):
//! `admin.token` (device owner or fleet administrator), `agent.token` (the
//! AI orchestrator), `user.token` (fleet mode only) and `intent.token`
//! (clients of the orchestrator; it carries no fluxd authority).
//!
//! Browsers pair instead of reading token files: the person enters the
//! one-time pairing code that fluxd prints to its log and keeps in
//! `secrets/pairing.code`, and receives a revocable session token. Only the
//! SHA-256 of a session token is stored. Codes are single use, rotate after
//! five wrong attempts, and pairing then locks for 30 seconds.
//!
//! The orchestrator can never approve: tier 2+ needs a person. Permissions
//! are evaluated on every call, never cached.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use flux_core::RiskTier;
use flux_core::policy::Mode;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::util::{new_id, now, random_hex, read_json, set_private, write_atomic, write_json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// The AI orchestrator.
    Agent,
    /// A person using the machine (in fleet mode, not an administrator).
    User,
    /// Device owner (personal mode) or fleet administrator.
    Admin,
}

#[derive(Debug, Clone, Serialize)]
pub struct Principal {
    pub id: String,
    pub role: Role,
    /// Paired session the request came from; `None` for static tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

impl Principal {
    pub fn is_human(&self) -> bool {
        self.role != Role::Agent
    }

    /// Highest tier this principal may approve.
    pub fn can_approve(&self, tier: RiskTier, mode: Mode) -> bool {
        match (self.role, mode) {
            (Role::Agent, _) => false,
            (_, _) if tier == RiskTier::Blocked => false,
            (Role::Admin, _) => true,
            (Role::User, Mode::Personal) => true,
            (Role::User, Mode::Fleet) => tier <= RiskTier::Low,
        }
    }

    /// Tier 0-1 changes apply without a separate approval (PRD 7.6).
    pub fn can_apply_without_approval(&self, tier: RiskTier) -> bool {
        tier <= RiskTier::Low
    }

    pub fn can_manage_profiles(&self) -> bool {
        self.is_human()
    }

    pub fn can_use_dev_tools(&self) -> bool {
        self.role == Role::Admin
    }

    pub fn can_manage_sessions(&self) -> bool {
        self.role == Role::Admin
    }
}

/// A paired browser. The token itself is never stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: String,
    pub name: String,
    pub role: Role,
    pub created_at: String,
    pub token_sha256: String,
}

impl SessionRecord {
    /// View without the token hash, for the API.
    pub fn public(&self) -> Value {
        json!({ "id": self.id, "name": self.name, "role": self.role, "created_at": self.created_at })
    }

    fn principal(&self) -> Principal {
        let base = match self.role {
            Role::Admin => "owner",
            Role::User => "local-user",
            Role::Agent => "agent",
        };
        Principal { id: format!("{base}@{}", self.id), role: self.role, session: Some(self.id.clone()) }
    }
}

pub const PAIR_ATTEMPTS: u32 = 5;
pub const PAIR_LOCKOUT: Duration = Duration::from_secs(30);
/// Crockford base32: no I, L, O or U, so codes read aloud unambiguously.
const CODE_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

struct Pairing {
    code: String,
    failures: u32,
    locked_until: Option<Instant>,
}

#[derive(Debug)]
pub enum PairError {
    WrongCode {
        attempts_left: u32,
    },
    /// Too many wrong attempts: the code was rotated and pairing is paused.
    Locked {
        retry_after_s: u64,
        rotated: bool,
    },
    Internal(anyhow::Error),
}

pub struct Paired {
    pub token: String,
    pub session: SessionRecord,
    pub principal: Principal,
}

/// 8 characters (40 bits) from the Crockford alphabet.
fn new_code() -> Result<String> {
    let mut bytes = [0u8; 5];
    getrandom::fill(&mut bytes).map_err(|e| anyhow!("random: {e}"))?;
    let v = bytes.iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b));
    Ok((0..8).rev().map(|i| CODE_ALPHABET[((v >> (i * 5)) & 31) as usize] as char).collect())
}

/// `K7QM3XRA` → `K7QM-3XRA`.
pub fn format_code(code: &str) -> String {
    if code.len() == 8 { format!("{}-{}", &code[..4], &code[4..]) } else { code.to_string() }
}

/// Accepts lower case, dashes and spaces, and the usual look-alikes (O→0, I/L→1).
pub fn normalize_code(input: &str) -> String {
    input
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        })
        .collect()
}

fn sha256_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && bool::from(a.ct_eq(b))
}

fn load_token(dir: &Path, file: &str, env: &str) -> Result<String> {
    if let Ok(v) = std::env::var(env)
        && !v.trim().is_empty()
    {
        return Ok(v.trim().to_string());
    }
    let path = dir.join(file);
    if path.exists() {
        return Ok(fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?.trim().to_string());
    }
    let token = format!("flx_{}", random_hex(32)?);
    write_atomic(&path, format!("{token}\n").as_bytes())?;
    set_private(&path)?;
    Ok(token)
}

pub struct Tokens {
    statics: Vec<(Vec<u8>, Principal)>,
    intent: String,
    sessions: RwLock<Vec<SessionRecord>>,
    sessions_path: PathBuf,
    pairing: Mutex<Pairing>,
    pairing_path: PathBuf,
}

impl Tokens {
    /// Loads tokens and paired sessions from `secrets/`, creating tokens on
    /// first boot, and issues a fresh pairing code.
    pub fn load_or_create(secrets: &Path, mode: Mode) -> Result<Self> {
        fs::create_dir_all(secrets)?;
        set_private(secrets)?;
        let principal = |id: &str, role| Principal { id: id.into(), role, session: None };
        let mut statics = vec![
            (load_token(secrets, "admin.token", "FLUX_ADMIN_TOKEN")?.into_bytes(), principal("owner", Role::Admin)),
            (
                load_token(secrets, "agent.token", "FLUX_AGENT_TOKEN")?.into_bytes(),
                principal("flux-agent", Role::Agent),
            ),
        ];
        if mode == Mode::Fleet {
            statics.push((
                load_token(secrets, "user.token", "FLUX_USER_TOKEN")?.into_bytes(),
                principal("local-user", Role::User),
            ));
        }
        let intent = load_token(secrets, "intent.token", "FLUX_INTENT_TOKEN")?;
        let sessions_path = secrets.join("sessions.json");
        let sessions: Vec<SessionRecord> = if sessions_path.exists() { read_json(&sessions_path)? } else { Vec::new() };
        let tokens = Self {
            statics,
            intent,
            sessions: RwLock::new(sessions),
            sessions_path,
            pairing: Mutex::new(Pairing { code: new_code()?, failures: 0, locked_until: None }),
            pairing_path: secrets.join("pairing.code"),
        };
        tokens.write_code(&tokens.pairing.lock().expect("pairing lock").code)?;
        Ok(tokens)
    }

    fn write_code(&self, code: &str) -> Result<()> {
        write_atomic(&self.pairing_path, format!("{}\n", format_code(code)).as_bytes())?;
        set_private(&self.pairing_path)
    }

    fn persist_sessions(&self, sessions: &[SessionRecord]) -> Result<()> {
        write_json(&self.sessions_path, &sessions)?;
        set_private(&self.sessions_path)
    }

    /// Constant-time lookup across static tokens and paired sessions.
    pub fn authenticate(&self, presented: &str) -> Option<Principal> {
        let p = presented.trim();
        let mut found: Option<Principal> = None;
        for (token, principal) in &self.statics {
            if ct_eq(token, p.as_bytes()) {
                found = Some(principal.clone());
            }
        }
        if found.is_none() && p.starts_with("flxs_") {
            let hash = sha256_hex(p);
            for s in self.sessions.read().expect("sessions lock").iter() {
                if ct_eq(s.token_sha256.as_bytes(), hash.as_bytes()) {
                    found = Some(s.principal());
                }
            }
        }
        found
    }

    /// Current pairing code, formatted for people (`K7QM-3XRA`).
    pub fn pairing_code(&self) -> String {
        format_code(&self.pairing.lock().expect("pairing lock").code)
    }

    pub fn intent_token(&self) -> &str {
        &self.intent
    }

    /// Exchanges a pairing code for a session token. Personal mode pairs the
    /// device owner; fleet mode pairs a local user (administrators use their
    /// own credentials).
    pub fn pair(&self, code: &str, name: &str, mode: Mode) -> Result<Paired, PairError> {
        let mut p = self.pairing.lock().expect("pairing lock");
        if let Some(until) = p.locked_until {
            let left = until.saturating_duration_since(Instant::now());
            if !left.is_zero() {
                return Err(PairError::Locked { retry_after_s: left.as_secs().max(1), rotated: false });
            }
            p.locked_until = None;
        }
        if !ct_eq(normalize_code(code).as_bytes(), p.code.as_bytes()) {
            p.failures += 1;
            if p.failures >= PAIR_ATTEMPTS {
                p.code = new_code().map_err(PairError::Internal)?;
                p.failures = 0;
                p.locked_until = Some(Instant::now() + PAIR_LOCKOUT);
                self.write_code(&p.code).map_err(PairError::Internal)?;
                return Err(PairError::Locked { retry_after_s: PAIR_LOCKOUT.as_secs(), rotated: true });
            }
            return Err(PairError::WrongCode { attempts_left: PAIR_ATTEMPTS - p.failures });
        }

        let token = format!("flxs_{}", random_hex(32).map_err(PairError::Internal)?);
        let name: String = name.chars().filter(|c| !c.is_control()).take(60).collect();
        let session = SessionRecord {
            id: new_id("se").chars().take(20).collect(),
            name: if name.trim().is_empty() { "Paired screen".into() } else { name.trim().to_string() },
            role: if mode == Mode::Fleet { Role::User } else { Role::Admin },
            created_at: now(),
            token_sha256: sha256_hex(&token),
        };
        {
            let mut sessions = self.sessions.write().expect("sessions lock");
            sessions.push(session.clone());
            self.persist_sessions(&sessions).map_err(PairError::Internal)?;
        }
        // Single use: the next screen needs a new code.
        p.code = new_code().map_err(PairError::Internal)?;
        p.failures = 0;
        self.write_code(&p.code).map_err(PairError::Internal)?;
        Ok(Paired { token, principal: session.principal(), session })
    }

    pub fn sessions(&self) -> Vec<SessionRecord> {
        self.sessions.read().expect("sessions lock").clone()
    }

    /// Revokes a paired session. Returns the removed record.
    pub fn revoke(&self, id: &str) -> Result<Option<SessionRecord>> {
        let mut sessions = self.sessions.write().expect("sessions lock");
        let Some(pos) = sessions.iter().position(|s| s.id == id) else { return Ok(None) };
        let removed = sessions.remove(pos);
        self.persist_sessions(&sessions)?;
        Ok(Some(removed))
    }

    pub fn static_count(&self) -> usize {
        self.statics.len()
    }

    pub fn session_count(&self) -> usize {
        self.sessions.read().expect("sessions lock").len()
    }
}
