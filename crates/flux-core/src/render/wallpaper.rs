//! Original, deterministic SVG wallpapers colored from the palette. No
//! third-party artwork is used, which keeps look-and-feel styles legally safe.

use std::fmt::Write;

use crate::color::{Hsl, Rgb, SplitMix64};
use crate::spec::{SystemSpec, WallpaperStyle};

const W: f64 = 1920.0;
const H: f64 = 1080.0;

pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

fn shift(hex: &str, dh: f64, dl: f64) -> String {
    let c = Rgb::from_hex(hex).unwrap_or(crate::color::BLACK).to_hsl();
    Hsl::new(c.h + dh, c.s, (c.l + dl).clamp(0.0, 1.0)).to_rgb().to_hex()
}

pub fn svg(spec: &SystemSpec) -> String {
    let p = &spec.design.tokens.palette;
    let seed = spec.desktop.wallpaper.seed as u64;
    let mut rng = SplitMix64::new(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0xF1u64);
    let mut s = String::new();
    let _ = write!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" preserveAspectRatio="xMidYMid slice">"#
    );
    let _ = write!(s, "<title>FluxOS {} wallpaper</title>", xml_escape(&spec.design.theme_name));
    let bg = &p.background;
    match spec.desktop.wallpaper.style {
        WallpaperStyle::Solid => {
            let _ = write!(s, r#"<rect width="100%" height="100%" fill="{bg}"/>"#);
        }
        WallpaperStyle::Gradient => {
            let angle = rng.range(15.0, 75.0);
            let _ = write!(
                s,
                r#"<defs><linearGradient id="g" gradientTransform="rotate({angle:.1} .5 .5)"><stop offset="0" stop-color="{}"/><stop offset=".55" stop-color="{bg}"/><stop offset="1" stop-color="{}"/></linearGradient></defs><rect width="100%" height="100%" fill="url(#g)"/>"#,
                shift(&p.accent, 0.0, -0.18),
                shift(&p.secondary, 0.0, -0.22)
            );
        }
        WallpaperStyle::Aurora | WallpaperStyle::Mesh => {
            let blobs = if spec.desktop.wallpaper.style == WallpaperStyle::Aurora { 4 } else { 6 };
            let blur = if spec.desktop.wallpaper.style == WallpaperStyle::Aurora { 140 } else { 90 };
            let _ = write!(
                s,
                r#"<defs><filter id="b" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="{blur}"/></filter>"#
            );
            let _ = write!(
                s,
                r#"<radialGradient id="v" cx=".5" cy=".5" r=".75"><stop offset=".55" stop-color="{bg}" stop-opacity="0"/><stop offset="1" stop-color="{bg}" stop-opacity=".85"/></radialGradient></defs>"#
            );
            let _ = write!(s, r#"<rect width="100%" height="100%" fill="{bg}"/><g filter="url(#b)" opacity=".85">"#);
            let colors = [
                p.accent.as_str(),
                p.secondary.as_str(),
                p.accent.as_str(),
                p.success.as_str(),
                p.secondary.as_str(),
                p.warning.as_str(),
            ];
            for (i, base) in colors.iter().take(blobs).enumerate() {
                let cx = rng.range(0.1, 0.9) * W;
                let cy = if spec.desktop.wallpaper.style == WallpaperStyle::Aurora {
                    rng.range(0.15, 0.6) * H
                } else {
                    rng.range(0.1, 0.9) * H
                };
                let rx = rng.range(0.18, 0.34) * W;
                let ry = rng.range(0.08, 0.2) * H;
                let rot = rng.range(-35.0, 35.0);
                let fill = shift(base, rng.range(-12.0, 12.0), -0.06 * i as f64 / blobs as f64);
                let op = rng.range(0.35, 0.7);
                let _ = write!(
                    s,
                    r#"<ellipse cx="{cx:.0}" cy="{cy:.0}" rx="{rx:.0}" ry="{ry:.0}" fill="{fill}" opacity="{op:.2}" transform="rotate({rot:.1} {cx:.0} {cy:.0})"/>"#
                );
            }
            let _ = write!(s, r#"</g><rect width="100%" height="100%" fill="url(#v)"/>"#);
        }
        WallpaperStyle::Grid => {
            let line = shift(&p.border, 0.0, 0.02);
            let glow = &p.accent;
            let step = 48.0;
            let _ = write!(
                s,
                r#"<defs><pattern id="p" width="{step}" height="{step}" patternUnits="userSpaceOnUse"><path d="M {step} 0 L 0 0 0 {step}" fill="none" stroke="{line}" stroke-width="1"/></pattern><radialGradient id="gl" cx="{:.2}" cy="{:.2}" r=".6"><stop offset="0" stop-color="{glow}" stop-opacity=".35"/><stop offset="1" stop-color="{bg}" stop-opacity="0"/></radialGradient></defs>"#,
                rng.range(0.25, 0.75),
                rng.range(0.25, 0.7)
            );
            let _ = write!(
                s,
                r#"<rect width="100%" height="100%" fill="{bg}"/><rect width="100%" height="100%" fill="url(#p)" opacity=".55"/><rect width="100%" height="100%" fill="url(#gl)"/>"#
            );
        }
    }
    s.push_str("</svg>\n");
    s
}
