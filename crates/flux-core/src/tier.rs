//! Permission tiers (PRD section 7.6).

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Risk tier of an operation or of a concrete change.
///
/// | Tier | Behavior |
/// |------|----------|
/// | 0 Safe | auto-apply with snapshot |
/// | 1 Low | auto-apply, undo offered |
/// | 2 Medium | preview and confirm |
/// | 3 High | explicit confirmation, sandbox test, rollback entry required |
/// | 4 Blocked | never allowed by the agent |
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RiskTier {
    Safe = 0,
    Low = 1,
    Medium = 2,
    High = 3,
    Blocked = 4,
}

impl RiskTier {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            0 => Self::Safe,
            1 => Self::Low,
            2 => Self::Medium,
            3 => Self::High,
            4 => Self::Blocked,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Blocked => "blocked",
        }
    }

    /// What the PRD says should happen for this tier.
    pub fn behavior(self) -> &'static str {
        match self {
            Self::Safe => "Auto-apply with snapshot",
            Self::Low => "Auto-apply, undo offered",
            Self::Medium => "Preview and confirm",
            Self::High => "Explicit confirmation, sandbox test, rollback entry required",
            Self::Blocked => "Never allowed by the agent",
        }
    }
}

impl Serialize for RiskTier {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u8(self.as_u8())
    }
}

impl<'de> Deserialize<'de> for RiskTier {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = u8::deserialize(d)?;
        RiskTier::from_u8(v).ok_or_else(|| serde::de::Error::custom(format!("invalid risk tier {v}")))
    }
}
