//! Principals, roles and bearer tokens.
//!
//! The AI orchestrator authenticates as `agent`. It may propose anything and
//! apply tier 0-1 changes, but it can never approve: tier 2+ needs a human
//! token. Permissions are evaluated on every call, never cached.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use flux_core::RiskTier;
use flux_core::policy::Mode;
use serde::Serialize;
use subtle::ConstantTimeEq;

use crate::util::{random_hex, set_private, write_atomic};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
}

pub struct Tokens {
    entries: Vec<(Vec<u8>, Principal)>,
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

impl Tokens {
    /// Loads tokens from `secrets/`, creating them on first boot.
    pub fn load_or_create(secrets: &Path, mode: Mode) -> Result<Self> {
        fs::create_dir_all(secrets)?;
        set_private(secrets)?;
        let mut entries = vec![
            (
                load_token(secrets, "admin.token", "FLUX_ADMIN_TOKEN")?.into_bytes(),
                Principal { id: "owner".into(), role: Role::Admin },
            ),
            (
                load_token(secrets, "agent.token", "FLUX_AGENT_TOKEN")?.into_bytes(),
                Principal { id: "flux-agent".into(), role: Role::Agent },
            ),
        ];
        if mode == Mode::Fleet {
            entries.push((
                load_token(secrets, "user.token", "FLUX_USER_TOKEN")?.into_bytes(),
                Principal { id: "local-user".into(), role: Role::User },
            ));
        }
        // Token the UI presents to the AI orchestrator. It carries no fluxd
        // authority, so the agent never needs to see a human credential.
        load_token(secrets, "intent.token", "FLUX_INTENT_TOKEN")?;
        Ok(Self { entries })
    }

    /// Constant-time lookup across all known tokens.
    pub fn authenticate(&self, presented: &str) -> Option<Principal> {
        let p = presented.trim().as_bytes();
        let mut found: Option<Principal> = None;
        for (token, principal) in &self.entries {
            if token.len() == p.len() && bool::from(token.as_slice().ct_eq(p)) {
                found = Some(principal.clone());
            }
        }
        found
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }
}
