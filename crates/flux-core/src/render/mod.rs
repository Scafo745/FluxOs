//! Deterministic renderers: one validated spec in, a tree of artifacts out.
//! The same tree is materialized by the sandbox target and embedded in the
//! NixOS module, so a preview shows exactly what will be applied.

pub mod nix;
pub mod theme;
pub mod wallpaper;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::spec::SystemSpec;
use crate::tokens;

/// One rendered file, relative to the target root.
#[derive(Debug, Clone, Serialize)]
pub struct Artifact {
    pub path: String,
    pub sha256: String,
    pub bytes: usize,
    #[serde(skip)]
    pub content: String,
}

impl Artifact {
    pub fn new(path: &str, content: String) -> Self {
        Self {
            path: path.to_string(),
            sha256: hex::encode(Sha256::digest(content.as_bytes())),
            bytes: content.len(),
            content,
        }
    }
}

/// Theme assets shared by every exporter.
pub struct Assets {
    pub spec_json: String,
    pub tokens_json: String,
    pub theme_css: String,
    pub gtk_css: String,
    pub gtk_settings: String,
    pub kde_scheme: String,
    pub kdeglobals: String,
    pub kwinrc: String,
    pub plasma_layout: String,
    pub hyprland: String,
    pub dconf: String,
    pub wallpaper_svg: String,
    pub sysctl_conf: String,
}

pub struct RenderContext<'a> {
    pub generation: u64,
    pub plan_id: Option<&'a str>,
}

fn sysctl_conf(spec: &SystemSpec, header: &str) -> String {
    let kernel = crate::catalog::kernel(spec.kernel.variant);
    let mut out = format!("# {header}\n");
    for (k, v) in kernel.extra_sysctl {
        if !spec.kernel.sysctl.contains_key(*k) {
            out.push_str(&format!("{k} = {v}\n"));
        }
    }
    for (k, v) in &spec.kernel.sysctl {
        out.push_str(&format!("{k} = {v}\n"));
    }
    out
}

pub fn assets(spec: &SystemSpec, ctx: &RenderContext<'_>) -> Assets {
    let header = format!(
        "FluxOS generation {} ({}){}",
        ctx.generation,
        spec.design.theme_name,
        ctx.plan_id.map(|p| format!(", plan {p}")).unwrap_or_default()
    );
    let a = &spec.accessibility;
    let dtcg = tokens::to_dtcg(&spec.design.tokens, a.effective_text_scale(), a.reduce_motion, &header);
    Assets {
        spec_json: serde_json::to_string_pretty(spec).unwrap_or_default() + "\n",
        tokens_json: serde_json::to_string_pretty(&dtcg).unwrap_or_default() + "\n",
        theme_css: theme::css(spec, &header),
        gtk_css: theme::gtk_css(spec, &header),
        gtk_settings: theme::gtk_settings_ini(spec),
        kde_scheme: theme::kde_scheme_file(spec),
        kdeglobals: theme::kdeglobals(spec),
        kwinrc: theme::kwinrc(spec),
        plasma_layout: theme::plasma_layout_js(spec, &format!("generation {}", ctx.generation)),
        hyprland: theme::hyprland(spec, &header),
        dconf: theme::dconf_keyfile(spec),
        wallpaper_svg: wallpaper::svg(spec),
        sysctl_conf: sysctl_conf(spec, &header),
    }
}

/// Renders the full artifact tree for a spec.
pub fn render_all(spec: &SystemSpec, ctx: &RenderContext<'_>) -> Vec<Artifact> {
    let a = assets(spec, ctx);
    let header = format!(
        "FluxOS generation {}{}: spec {}",
        ctx.generation,
        ctx.plan_id.map(|p| format!(", plan {p}")).unwrap_or_default(),
        spec.digest()
    );
    let module = nix::module(spec, &a, &nix::NixContext { generation: ctx.generation, header: &header });
    vec![
        Artifact::new("etc/nixos/fluxos/generated.nix", module),
        Artifact::new("etc/fluxos/spec.json", a.spec_json.clone()),
        Artifact::new("etc/fluxos/tokens.json", a.tokens_json.clone()),
        Artifact::new("etc/fluxos/theme.css", a.theme_css.clone()),
        Artifact::new("etc/fluxos/wallpaper.svg", a.wallpaper_svg.clone()),
        Artifact::new("etc/fluxos/gtk-4.0.css", a.gtk_css.clone()),
        Artifact::new("etc/xdg/gtk-4.0/settings.ini", a.gtk_settings.clone()),
        Artifact::new("etc/xdg/kdeglobals", a.kdeglobals.clone()),
        Artifact::new("etc/xdg/kwinrc", a.kwinrc.clone()),
        Artifact::new("usr/share/color-schemes/FluxOS.colors", a.kde_scheme.clone()),
        Artifact::new("usr/share/plasma/layout/fluxos-layout.js", a.plasma_layout.clone()),
        Artifact::new("etc/xdg/hypr/fluxos.conf", a.hyprland.clone()),
        Artifact::new("etc/dconf/db/local.d/00-fluxos", a.dconf.clone()),
        Artifact::new("etc/sysctl.d/90-fluxos.conf", a.sysctl_conf.clone()),
    ]
}
