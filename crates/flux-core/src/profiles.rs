//! Industry profiles. A profile document is a JSON merge-patch overlay on
//! the FluxOS default spec, plus optional palette and style seeds. It is
//! materialized into a full, validated [`SystemSpec`] at load time.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::color::Rgb;
use crate::ops::apply_style_preset;
use crate::spec::{StylePreset, SystemSpec, merge_patch};
use crate::tokens::{ColorScheme, ContrastLevel, generate_palette};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaletteSeed {
    pub accent: String,
    pub secondary: Option<String>,
    pub scheme: ColorScheme,
    #[serde(default)]
    pub contrast: ContrastLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileDoc {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub vertical: Option<String>,
    pub summary: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// `stable` or `preview`.
    #[serde(default = "default_status")]
    pub status: String,
    /// Regulated verticals must be reviewed by the customer's compliance team.
    #[serde(default)]
    pub compliance_review_required: bool,
    /// Light integrations shipped with the profile (informational).
    #[serde(default)]
    pub integrations: Vec<String>,
    #[serde(default)]
    pub style: Option<StylePreset>,
    #[serde(default)]
    pub palette: Option<PaletteSeed>,
    #[serde(default)]
    pub overlay: Value,
}

fn default_status() -> String {
    "stable".into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileSource {
    Builtin,
    User,
    Imported,
}

#[derive(Debug, Clone, Serialize)]
pub struct Profile {
    #[serde(flatten)]
    pub doc: ProfileDoc,
    pub source: ProfileSource,
    pub spec: SystemSpec,
}

impl Profile {
    /// Summary without the (large) overlay and spec.
    pub fn summary_json(&self) -> Value {
        serde_json::json!({
            "id": self.doc.id,
            "name": self.doc.name,
            "version": self.doc.version,
            "vertical": self.doc.vertical,
            "summary": self.doc.summary,
            "tags": self.doc.tags,
            "status": self.doc.status,
            "compliance_review_required": self.doc.compliance_review_required,
            "integrations": self.doc.integrations,
            "source": self.source,
            "theme_name": self.spec.design.theme_name,
            "desktop": self.spec.desktop.environment,
            "style": self.spec.desktop.style,
            "kernel": self.spec.kernel.variant,
            "accent": self.spec.design.tokens.palette.accent,
        })
    }
}

/// Materializes a profile document into a full spec.
pub fn materialize(doc: &ProfileDoc) -> Result<SystemSpec, String> {
    let mut spec = SystemSpec::default();
    if let Some(style) = doc.style {
        apply_style_preset(&mut spec, style);
    }
    if let Some(seed) = &doc.palette {
        let accent =
            Rgb::from_hex(&seed.accent).ok_or_else(|| format!("profile {}: invalid accent {}", doc.id, seed.accent))?;
        let secondary = match &seed.secondary {
            Some(s) => Some(Rgb::from_hex(s).ok_or_else(|| format!("profile {}: invalid secondary {s}", doc.id))?),
            None => None,
        };
        spec.design.tokens.scheme = seed.scheme;
        spec.design.tokens.palette = generate_palette(accent, secondary, seed.scheme, seed.contrast);
    }
    let mut value = serde_json::to_value(&spec).map_err(|e| e.to_string())?;
    if !doc.overlay.is_null() {
        merge_patch(&mut value, &doc.overlay);
    }
    serde_json::from_value(value).map_err(|e| format!("profile {}: {e}", doc.id))
}

const BUILTIN: &[(&str, &str)] = &[
    ("default", include_str!("../../../profiles/default.json")),
    ("gaming", include_str!("../../../profiles/gaming.json")),
    ("education", include_str!("../../../profiles/education.json")),
    ("telecom", include_str!("../../../profiles/telecom.json")),
    ("finance", include_str!("../../../profiles/finance.json")),
    ("healthcare", include_str!("../../../profiles/healthcare.json")),
];

#[derive(Debug, Clone, Default)]
pub struct ProfileSet {
    map: BTreeMap<String, Profile>,
}

impl ProfileSet {
    /// Loads the built-in profiles. Panics only if a shipped profile is invalid,
    /// which is a build defect caught at first boot.
    pub fn builtin() -> Result<Self, String> {
        let mut set = Self::default();
        for (name, src) in BUILTIN {
            let doc: ProfileDoc = serde_json::from_str(src).map_err(|e| format!("built-in profile {name}: {e}"))?;
            set.insert(doc, ProfileSource::Builtin)?;
        }
        Ok(set)
    }

    pub fn insert(&mut self, doc: ProfileDoc, source: ProfileSource) -> Result<&Profile, String> {
        if !valid_id(&doc.id) {
            return Err(format!("invalid profile id {:?} (use a-z, 0-9 and '-')", doc.id));
        }
        if source != ProfileSource::Builtin && self.map.get(&doc.id).is_some_and(|p| p.source == ProfileSource::Builtin)
        {
            return Err(format!("profile id {:?} is reserved by a built-in profile", doc.id));
        }
        let spec = materialize(&doc)?;
        let id = doc.id.clone();
        self.map.insert(id.clone(), Profile { doc, source, spec });
        Ok(&self.map[&id])
    }

    pub fn get(&self, id: &str) -> Option<&Profile> {
        self.map.get(id)
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.map.keys().map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Profile> {
        self.map.values()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 48
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.starts_with('-')
}
