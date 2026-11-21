//! Color math: hex parsing, HSL, WCAG 2.x contrast and palette generation.

/// sRGB color with components in `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgb {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

/// HSL color: hue in degrees `0..360`, saturation and lightness in `0..=1`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hsl {
    pub h: f64,
    pub s: f64,
    pub l: f64,
}

pub const WHITE: Rgb = Rgb { r: 1.0, g: 1.0, b: 1.0 };
pub const BLACK: Rgb = Rgb { r: 0.0, g: 0.0, b: 0.0 };

fn channel(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

impl Rgb {
    pub fn from_u8(r: u8, g: u8, b: u8) -> Self {
        Self { r: r as f64 / 255.0, g: g as f64 / 255.0, b: b as f64 / 255.0 }
    }

    /// Parses `#rgb` or `#rrggbb` (case-insensitive).
    pub fn from_hex(s: &str) -> Option<Self> {
        let hex = s.trim().strip_prefix('#')?;
        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Self::from_u8(r, g, b))
            }
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
                Some(Self::from_u8(r, g, b))
            }
            _ => None,
        }
    }

    pub fn to_u8(self) -> (u8, u8, u8) {
        (channel(self.r), channel(self.g), channel(self.b))
    }

    /// Lowercase `#rrggbb`.
    pub fn to_hex(self) -> String {
        let (r, g, b) = self.to_u8();
        format!("#{r:02x}{g:02x}{b:02x}")
    }

    /// Components rounded to 4 decimals, as used by DTCG `components`.
    pub fn components(self) -> [f64; 3] {
        let (r, g, b) = self.to_u8();
        let q = |v: u8| ((v as f64 / 255.0) * 10_000.0).round() / 10_000.0;
        [q(r), q(g), q(b)]
    }

    /// WCAG 2.x relative luminance.
    pub fn luminance(self) -> f64 {
        fn lin(c: f64) -> f64 {
            if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        }
        let (r, g, b) = self.to_u8();
        let (r, g, b) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
        0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
    }

    pub fn mix(self, other: Rgb, t: f64) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        Rgb {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
        }
    }

    pub fn to_hsl(self) -> Hsl {
        let (r, g, b) = (self.r, self.g, self.b);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        let d = max - min;
        if d.abs() < f64::EPSILON {
            return Hsl { h: 0.0, s: 0.0, l };
        }
        let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
        let h = if (max - r).abs() < f64::EPSILON {
            ((g - b) / d) + if g < b { 6.0 } else { 0.0 }
        } else if (max - g).abs() < f64::EPSILON {
            ((b - r) / d) + 2.0
        } else {
            ((r - g) / d) + 4.0
        };
        Hsl { h: (h * 60.0).rem_euclid(360.0), s, l }
    }
}

impl Hsl {
    pub fn new(h: f64, s: f64, l: f64) -> Self {
        Self { h: h.rem_euclid(360.0), s: s.clamp(0.0, 1.0), l: l.clamp(0.0, 1.0) }
    }

    pub fn to_rgb(self) -> Rgb {
        let Hsl { h, s, l } = self;
        if s <= 0.0 {
            return Rgb { r: l, g: l, b: l };
        }
        let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
        let p = 2.0 * l - q;
        let hk = h / 360.0;
        let f = |t: f64| {
            let t = t.rem_euclid(1.0);
            if t < 1.0 / 6.0 {
                p + (q - p) * 6.0 * t
            } else if t < 0.5 {
                q
            } else if t < 2.0 / 3.0 {
                p + (q - p) * (2.0 / 3.0 - t) * 6.0
            } else {
                p
            }
        };
        Rgb { r: f(hk + 1.0 / 3.0), g: f(hk), b: f(hk - 1.0 / 3.0) }
    }
}

/// WCAG contrast ratio between two colors (1.0 ..= 21.0).
pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (a.luminance(), b.luminance());
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

pub fn contrast_hex(a: &str, b: &str) -> Option<f64> {
    Some(contrast(Rgb::from_hex(a)?, Rgb::from_hex(b)?))
}

/// Normalizes a valid hex color to lowercase `#rrggbb`.
pub fn normalize_hex(s: &str) -> Option<String> {
    Rgb::from_hex(s).map(Rgb::to_hex)
}

pub fn is_hex6(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Moves `c`'s lightness away from `bg` until the contrast reaches `min`.
pub fn ensure_contrast(c: Rgb, bg: Rgb, min: f64) -> Rgb {
    if contrast(c, bg) >= min {
        return c;
    }
    let lighten = bg.luminance() < 0.4;
    let mut hsl = c.to_hsl();
    for _ in 0..100 {
        hsl.l = if lighten { (hsl.l + 0.01).min(1.0) } else { (hsl.l - 0.01).max(0.0) };
        let rgb = hsl.to_rgb();
        if contrast(rgb, bg) >= min {
            return rgb;
        }
    }
    if lighten { WHITE } else { BLACK }
}

/// White or near-black, whichever reads better on `bg`.
pub fn best_text_on(bg: Rgb) -> Rgb {
    let ink = Rgb::from_u8(0x0b, 0x0b, 0x10);
    if contrast(WHITE, bg) >= contrast(ink, bg) { WHITE } else { ink }
}

/// Nearest GNOME (47+) accent color name for a hue.
pub fn gnome_accent_name(c: Rgb) -> &'static str {
    let hsl = c.to_hsl();
    if hsl.s < 0.15 {
        return "slate";
    }
    let anchors: [(&str, f64); 8] = [
        ("red", 0.0),
        ("orange", 28.0),
        ("yellow", 50.0),
        ("green", 130.0),
        ("teal", 180.0),
        ("blue", 215.0),
        ("purple", 270.0),
        ("pink", 325.0),
    ];
    let mut best = ("blue", f64::MAX);
    for (name, hue) in anchors {
        let d = (hsl.h - hue).abs();
        let d = d.min(360.0 - d);
        if d < best.1 {
            best = (name, d);
        }
    }
    best.0
}

/// Small deterministic PRNG (splitmix64) for generated assets.
#[derive(Debug, Clone)]
pub struct SplitMix64(u64);

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0.0..1.0`.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }
}
