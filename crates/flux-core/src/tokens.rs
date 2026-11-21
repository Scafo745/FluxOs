//! Design tokens: typed model, contrast-safe palette generation and the
//! W3C Design Tokens (DTCG 2025.10) JSON export.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::color::{self, Hsl, Rgb, best_text_on, ensure_contrast};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ColorScheme {
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ContrastLevel {
    #[default]
    Normal,
    High,
}

/// Semantic color palette. Every value is a lowercase `#rrggbb` hex string.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Palette {
    pub background: String,
    pub surface: String,
    pub surface_raised: String,
    pub border: String,
    pub text: String,
    pub text_muted: String,
    pub accent: String,
    pub accent_text: String,
    pub secondary: String,
    pub success: String,
    pub warning: String,
    pub danger: String,
}

impl Palette {
    pub fn entries(&self) -> [(&'static str, &String); 12] {
        [
            ("background", &self.background),
            ("surface", &self.surface),
            ("surface_raised", &self.surface_raised),
            ("border", &self.border),
            ("text", &self.text),
            ("text_muted", &self.text_muted),
            ("accent", &self.accent),
            ("accent_text", &self.accent_text),
            ("secondary", &self.secondary),
            ("success", &self.success),
            ("warning", &self.warning),
            ("danger", &self.danger),
        ]
    }

    pub fn entries_mut(&mut self) -> [(&'static str, &mut String); 12] {
        [
            ("background", &mut self.background),
            ("surface", &mut self.surface),
            ("surface_raised", &mut self.surface_raised),
            ("border", &mut self.border),
            ("text", &mut self.text),
            ("text_muted", &mut self.text_muted),
            ("accent", &mut self.accent),
            ("accent_text", &mut self.accent_text),
            ("secondary", &mut self.secondary),
            ("success", &mut self.success),
            ("warning", &mut self.warning),
            ("danger", &mut self.danger),
        ]
    }

    pub fn rgb(&self, hex: &str) -> Rgb {
        Rgb::from_hex(hex).unwrap_or(color::BLACK)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Typography {
    /// UI font family (must be in the font catalog).
    pub font_sans: String,
    /// Monospace font family (terminals, code).
    pub font_mono: String,
    /// Display font family (headings, boot screen, clocks).
    pub font_display: String,
    /// Base UI text size in px (11-24).
    pub base_size_px: u8,
    /// Modular type scale ratio between heading steps (1.05-1.5).
    pub scale_ratio: f64,
    /// Unitless body line height (1.1-2.0).
    pub line_height: f64,
    pub weight_regular: u16,
    pub weight_bold: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Shape {
    pub radius_sm_px: u8,
    pub radius_md_px: u8,
    pub radius_lg_px: u8,
    /// Base spacing unit in px; the spacing scale is built from multiples of it.
    pub spacing_unit_px: u8,
    pub border_width_px: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Motion {
    pub duration_fast_ms: u16,
    pub duration_base_ms: u16,
    pub duration_slow_ms: u16,
    /// CSS-style cubic-bezier control points [x1, y1, x2, y2]; x values in 0..1.
    pub easing: [f64; 4],
    /// Global animation multiplier: 0 disables animations, 1 is normal, 2 is twice as slow.
    pub animation_scale: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Effects {
    /// Background blur radius for translucent surfaces, in px (0-64).
    pub blur_px: u8,
    /// Surface transparency (0 = opaque, max 0.6).
    pub transparency: f64,
    /// Drop shadow opacity (0-0.8).
    pub shadow_opacity: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesignTokens {
    pub scheme: ColorScheme,
    pub palette: Palette,
    pub typography: Typography,
    pub shape: Shape,
    pub motion: Motion,
    pub effects: Effects,
}

/// Generates a complete palette from a brand/accent color.
///
/// The result always meets WCAG AA for body text (4.5:1) and 3:1 for UI
/// accents; with [`ContrastLevel::High`] text meets AAA (7:1).
pub fn generate_palette(accent: Rgb, secondary: Option<Rgb>, scheme: ColorScheme, contrast: ContrastLevel) -> Palette {
    let a = accent.to_hsl();
    let tint = (a.s * 0.35).min(0.28);
    let h = |hh: f64, s: f64, l: f64| Hsl::new(hh, s, l).to_rgb();

    let (bg, surface, raised, border, text, muted) = match (scheme, contrast) {
        (ColorScheme::Dark, ContrastLevel::Normal) => (
            h(a.h, tint, 0.065),
            h(a.h, tint, 0.105),
            h(a.h, tint, 0.145),
            h(a.h, tint * 0.8, 0.23),
            h(a.h, 0.18, 0.95),
            h(a.h, 0.10, 0.70),
        ),
        (ColorScheme::Dark, ContrastLevel::High) => (
            h(a.h, 0.0, 0.0),
            h(a.h, 0.10, 0.06),
            h(a.h, 0.10, 0.10),
            h(a.h, 0.10, 0.60),
            color::WHITE,
            h(a.h, 0.05, 0.86),
        ),
        (ColorScheme::Light, ContrastLevel::Normal) => (
            h(a.h, tint * 0.6, 0.975),
            color::WHITE,
            h(a.h, tint * 0.6, 0.945),
            h(a.h, tint * 0.5, 0.86),
            h(a.h, 0.25, 0.10),
            h(a.h, 0.12, 0.36),
        ),
        (ColorScheme::Light, ContrastLevel::High) => {
            (color::WHITE, h(a.h, 0.0, 0.97), h(a.h, 0.0, 0.93), h(a.h, 0.10, 0.30), color::BLACK, h(a.h, 0.10, 0.18))
        }
    };

    let (text_min, ui_min) = match contrast {
        ContrastLevel::Normal => (4.5, 3.0),
        ContrastLevel::High => (7.0, 4.5),
    };
    let text = ensure_contrast(text, bg, text_min);
    // Muted text is still body text, so it gets the same minimum as `text`.
    let muted = ensure_contrast(muted, bg, text_min);
    let accent_c = ensure_contrast(accent, bg, ui_min);
    let accent_text = best_text_on(accent_c);
    let secondary_c = secondary.unwrap_or_else(|| h(a.h + 150.0, (a.s * 0.9).max(0.45), a.l.clamp(0.45, 0.6)));
    let secondary_c = ensure_contrast(secondary_c, bg, ui_min);

    let status_l = if scheme == ColorScheme::Dark { 0.58 } else { 0.40 };
    let success = ensure_contrast(h(145.0, 0.60, status_l), bg, ui_min);
    let warning = ensure_contrast(h(38.0, 0.92, if scheme == ColorScheme::Dark { 0.58 } else { 0.42 }), bg, ui_min);
    let danger = ensure_contrast(h(355.0, 0.78, status_l), bg, ui_min);

    Palette {
        background: bg.to_hex(),
        surface: surface.to_hex(),
        surface_raised: raised.to_hex(),
        border: border.to_hex(),
        text: text.to_hex(),
        text_muted: muted.to_hex(),
        accent: accent_c.to_hex(),
        accent_text: accent_text.to_hex(),
        secondary: secondary_c.to_hex(),
        success: success.to_hex(),
        warning: warning.to_hex(),
        danger: danger.to_hex(),
    }
}

impl DesignTokens {
    /// Type scale in px: [xs, sm, base, lg, xl, 2xl, 3xl], after accessibility scaling.
    pub fn type_scale(&self, text_scale: f64) -> [(&'static str, f64); 7] {
        let base = self.typography.base_size_px as f64 * text_scale;
        let r = self.typography.scale_ratio;
        let px = |steps: i32| ((base * r.powi(steps)) * 10.0).round() / 10.0;
        [("xs", px(-2)), ("sm", px(-1)), ("base", px(0)), ("lg", px(1)), ("xl", px(2)), ("2xl", px(3)), ("3xl", px(4))]
    }

    pub fn spacing_scale(&self) -> [(&'static str, u32); 8] {
        let u = self.shape.spacing_unit_px as u32;
        [("1", u), ("2", u * 2), ("3", u * 3), ("4", u * 4), ("5", u * 5), ("6", u * 6), ("8", u * 8), ("12", u * 12)]
    }

    /// Durations after `animation_scale` and the reduce-motion preference.
    pub fn effective_durations(&self, reduce_motion: bool) -> [(&'static str, u32); 3] {
        let scale = if reduce_motion { 0.0 } else { self.motion.animation_scale };
        let d = |ms: u16| (ms as f64 * scale).round() as u32;
        [
            ("fast", d(self.motion.duration_fast_ms)),
            ("base", d(self.motion.duration_base_ms)),
            ("slow", d(self.motion.duration_slow_ms)),
        ]
    }
}

fn dtcg_color(hex: &str, alpha: Option<f64>) -> Value {
    let rgb = Rgb::from_hex(hex).unwrap_or(color::BLACK);
    let mut v = json!({
        "colorSpace": "srgb",
        "components": rgb.components(),
        "hex": rgb.to_hex(),
    });
    if let Some(a) = alpha {
        v["alpha"] = json!((a * 1000.0).round() / 1000.0);
    }
    v
}

fn dim(value: f64, unit: &str) -> Value {
    json!({ "value": value, "unit": unit })
}

/// Exports tokens in the W3C DTCG 2025.10 format (`.tokens.json`).
pub fn to_dtcg(tokens: &DesignTokens, text_scale: f64, reduce_motion: bool, description: &str) -> Value {
    let mut colors = Map::new();
    colors.insert("$type".into(), json!("color"));
    for (name, hex) in tokens.palette.entries() {
        colors.insert(name.replace('_', "-"), json!({ "$value": dtcg_color(hex, None) }));
    }
    let glass_alpha = 1.0 - tokens.effects.transparency;
    colors.insert(
        "glass".into(),
        json!({ "$value": dtcg_color(&tokens.palette.surface, Some(glass_alpha)), "$description": "Translucent surface for docks, panels and overlays" }),
    );

    let mut sizes = Map::new();
    sizes.insert("$type".into(), json!("dimension"));
    for (name, px) in tokens.type_scale(text_scale) {
        sizes.insert(name.into(), json!({ "$value": dim(px, "px") }));
    }

    let mut space = Map::new();
    space.insert("$type".into(), json!("dimension"));
    for (name, px) in tokens.spacing_scale() {
        space.insert(name.into(), json!({ "$value": dim(px as f64, "px") }));
    }

    let mut durations = Map::new();
    durations.insert("$type".into(), json!("duration"));
    for (name, ms) in tokens.effective_durations(reduce_motion) {
        durations.insert(name.into(), json!({ "$value": { "value": ms, "unit": "ms" } }));
    }

    let t = &tokens.typography;
    let shadow_alpha = tokens.effects.shadow_opacity;
    json!({
        "$description": description,
        "color": Value::Object(colors),
        "font": {
            "family": {
                "$type": "fontFamily",
                "sans": { "$value": [t.font_sans, "sans-serif"] },
                "mono": { "$value": [t.font_mono, "monospace"] },
                "display": { "$value": [t.font_display, t.font_sans, "sans-serif"] }
            },
            "weight": {
                "$type": "fontWeight",
                "regular": { "$value": t.weight_regular },
                "bold": { "$value": t.weight_bold }
            },
            "size": Value::Object(sizes),
            "line-height": { "$type": "number", "body": { "$value": t.line_height } }
        },
        "typography": {
            "$type": "typography",
            "body": { "$value": {
                "fontFamily": "{font.family.sans}",
                "fontSize": "{font.size.base}",
                "fontWeight": "{font.weight.regular}",
                "letterSpacing": dim(0.0, "px"),
                "lineHeight": t.line_height
            }},
            "heading": { "$value": {
                "fontFamily": "{font.family.display}",
                "fontSize": "{font.size.2xl}",
                "fontWeight": "{font.weight.bold}",
                "letterSpacing": dim(-0.2, "px"),
                "lineHeight": 1.2
            }},
            "code": { "$value": {
                "fontFamily": "{font.family.mono}",
                "fontSize": "{font.size.sm}",
                "fontWeight": "{font.weight.regular}",
                "letterSpacing": dim(0.0, "px"),
                "lineHeight": 1.45
            }}
        },
        "space": Value::Object(space),
        "radius": {
            "$type": "dimension",
            "sm": { "$value": dim(tokens.shape.radius_sm_px as f64, "px") },
            "md": { "$value": dim(tokens.shape.radius_md_px as f64, "px") },
            "lg": { "$value": dim(tokens.shape.radius_lg_px as f64, "px") }
        },
        "border": {
            "width": { "$type": "dimension", "default": { "$value": dim(tokens.shape.border_width_px as f64, "px") } }
        },
        "motion": {
            "duration": Value::Object(durations),
            "easing": {
                "$type": "cubicBezier",
                "standard": { "$value": tokens.motion.easing }
            }
        },
        "effect": {
            "blur": { "$type": "dimension", "glass": { "$value": dim(tokens.effects.blur_px as f64, "px") } },
            "opacity": { "$type": "number", "glass": { "$value": (glass_alpha * 1000.0).round() / 1000.0 } }
        },
        "shadow": {
            "$type": "shadow",
            "raised": { "$value": {
                "color": dtcg_color("#000000", Some(shadow_alpha)),
                "offsetX": dim(0.0, "px"),
                "offsetY": dim(8.0, "px"),
                "blur": dim(24.0, "px"),
                "spread": dim(0.0, "px")
            }}
        }
    })
}

/// Checks a DTCG document: every token (object with `$value`) must resolve a type.
pub fn check_dtcg(doc: &Value) -> Result<usize, String> {
    fn walk(v: &Value, inherited: Option<&str>, path: &str, count: &mut usize) -> Result<(), String> {
        let Some(obj) = v.as_object() else { return Ok(()) };
        let ty = obj.get("$type").and_then(Value::as_str).or(inherited);
        if obj.contains_key("$value") {
            if ty.is_none() {
                return Err(format!("token {path} has no resolvable $type"));
            }
            *count += 1;
            return Ok(());
        }
        for (k, child) in obj {
            if k.starts_with('$') {
                continue;
            }
            if k.contains(['{', '}', '.']) {
                return Err(format!("invalid token name {k:?} at {path}"));
            }
            walk(child, ty, &format!("{path}.{k}"), count)?;
        }
        Ok(())
    }
    let mut count = 0;
    walk(doc, None, "", &mut count)?;
    Ok(count)
}
