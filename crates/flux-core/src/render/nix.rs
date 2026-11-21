//! Renders a [`SystemSpec`] into a NixOS module.
//!
//! The module is imported by the host configuration (see `nix/module.nix`)
//! and rebuilt with `nixos-rebuild`. Every value comes from validated,
//! cataloged data, and all strings go through Nix escaping.

use std::collections::BTreeSet;
use std::fmt::Write;

use crate::catalog::{self, NixInstall};
use crate::spec::*;

use super::Assets;
use super::theme::{Dconf, gnome_dconf};

/// Nix double-quoted string literal.
pub fn q(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '$' => o.push_str("\\$"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// Nix indented string (`'' ... ''`) holding `content` verbatim.
/// `indent` is the indentation of the line that contains the opening quote.
pub fn ind(content: &str, indent: &str) -> String {
    let escaped = content.replace("''", "'''").replace("${", "''${").replace('\t', "  ");
    let inner = format!("{indent}  ");
    let mut o = String::from("''\n");
    for line in escaped.lines() {
        if !line.is_empty() {
            o.push_str(&inner);
            o.push_str(line);
        }
        o.push('\n');
    }
    o.push_str(&inner);
    o.push_str("''");
    o
}

fn list(items: impl IntoIterator<Item = String>) -> String {
    let v: Vec<String> = items.into_iter().collect();
    if v.is_empty() { "[ ]".into() } else { format!("[ {} ]", v.join(" ")) }
}

fn qlist<'a>(items: impl IntoIterator<Item = &'a str>) -> String {
    list(items.into_iter().map(q))
}

fn dconf_value(v: &Dconf) -> String {
    match v {
        Dconf::Str(s) => q(s),
        Dconf::Bool(b) => b.to_string(),
        // NixOS 26.05 refuses untyped integers in dconf settings.
        Dconf::Int(i) => format!("(lib.gvariant.mkInt32 ({i}))"),
        Dconf::Uint(u) => format!("(lib.gvariant.mkUint32 {u})"),
        Dconf::Double(d) => {
            let s = format!("{d}");
            if s.contains('.') { s } else { format!("{s}.0") }
        }
        Dconf::StrList(items) => qlist(items.iter().map(String::as_str)),
    }
}

fn pkg_expr(attr: &str) -> String {
    format!("pkgs.{attr}")
}

/// Unfree package names the spec needs (for `allowUnfreePredicate`).
pub fn unfree_names(spec: &SystemSpec) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for id in &spec.packages.system {
        if let Some(p) = catalog::package(id) {
            names.extend(p.unfree.iter().map(|s| s.to_string()));
        }
    }
    if matches!(spec.drivers.gpu, GpuDriver::Nvidia | GpuDriver::NvidiaOpen) {
        names.extend(["nvidia-x11", "nvidia-settings", "nvidia-persistenced"].map(String::from));
    }
    names
}

pub struct NixContext<'a> {
    pub generation: u64,
    pub header: &'a str,
}

pub fn module(spec: &SystemSpec, assets: &Assets, ctx: &NixContext<'_>) -> String {
    let mut s = String::new();
    let kernel = catalog::kernel(spec.kernel.variant);
    let kiosk = matches!(spec.session.mode, SessionMode::Kiosk | SessionMode::Exam);
    let gen_tag = format!("fluxos-gen-{}", ctx.generation);

    let _ = writeln!(s, "# {}", ctx.header);
    let _ =
        writeln!(s, "# DO NOT EDIT. fluxd rewrites this file for every generation; change the system through FluxOS.");
    let _ = writeln!(s, "{{ config, lib, pkgs, ... }}:");
    let _ = writeln!(s);
    let _ = writeln!(s, "let");
    // Theme assets packaged so desktops find them under /run/current-system/sw/share.
    let _ = writeln!(
        s,
        "  fluxosColorScheme = pkgs.writeTextDir \"share/color-schemes/FluxOS.colors\" {};",
        ind(&assets.kde_scheme, "  ")
    );
    let _ = writeln!(
        s,
        "  fluxosPlasmaLayout = pkgs.writeTextDir \"share/plasma/shells/org.kde.plasma.desktop/contents/updates/fluxos-layout-{gen_tag}.js\" {};",
        ind(&assets.plasma_layout, "  ")
    );
    if kiosk && let Some(url) = &spec.session.kiosk_url {
        let _ = writeln!(s, "  fluxosKiosk = pkgs.writeShellScript \"fluxos-kiosk\" ''");
        let _ = writeln!(
            s,
            "    exec ${{pkgs.firefox}}/bin/firefox --kiosk --private-window ${{lib.escapeShellArg {}}}",
            q(url)
        );
        let _ = writeln!(s, "  '';");
    }
    let _ = writeln!(s, "in");
    let _ = writeln!(s, "{{");

    // ---- base
    let _ = writeln!(s, "  # Base");
    let _ = writeln!(s, "  networking.hostName = {};", q(&spec.base.hostname));
    let _ = writeln!(s, "  networking.networkmanager.enable = true;");
    let _ = writeln!(s, "  time.timeZone = {};", q(&spec.base.timezone));
    let _ = writeln!(s, "  i18n.defaultLocale = {};", q(&spec.base.locale));
    let _ = writeln!(s, "  system.nixos.tags = [ {} ];", q(&gen_tag));
    let _ = writeln!(s);

    // ---- boot and kernel
    let _ = writeln!(s, "  # Boot and kernel ({})", kernel.label);
    let _ = writeln!(s, "  boot.kernelPackages = pkgs.{};", kernel.nix_packages);
    let mut params: Vec<&str> = kernel.extra_params.to_vec();
    if spec.boot.splash {
        params.extend(["quiet", "splash", "loglevel=3", "rd.udev.log_level=3", "udev.log_priority=3"]);
    }
    let _ = writeln!(s, "  boot.kernelParams = {};", qlist(params.iter().copied()));
    let _ = writeln!(s, "  boot.loader.timeout = {};", spec.boot.loader_timeout_s);
    if spec.boot.splash {
        let _ = writeln!(s, "  boot.consoleLogLevel = 3;");
        let _ = writeln!(s, "  boot.initrd.verbose = false;");
        let theme = &spec.boot.splash_theme;
        if catalog::SPLASH_ADI1090X.contains(&theme.as_str()) {
            let _ = writeln!(
                s,
                "  boot.plymouth = {{ enable = true; theme = {}; themePackages = [ (pkgs.adi1090x-plymouth-themes.override {{ selected_themes = [ {} ]; }}) ]; }};",
                q(theme),
                q(theme)
            );
        } else {
            let _ = writeln!(s, "  boot.plymouth = {{ enable = true; theme = {}; }};", q(theme));
        }
    }
    // Catalog sysctls first, explicit spec values win.
    let mut sysctl: Vec<(String, String)> = kernel
        .extra_sysctl
        .iter()
        .filter(|(k, _)| !spec.kernel.sysctl.contains_key(*k))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    for (k, v) in &spec.kernel.sysctl {
        let val = match v {
            SysctlValue::Int(i) => i.to_string(),
            SysctlValue::Text(t) => q(t),
        };
        sysctl.push((k.clone(), val));
    }
    sysctl.sort();
    if !sysctl.is_empty() {
        let _ = writeln!(s, "  boot.kernel.sysctl = {{");
        for (k, v) in &sysctl {
            let _ = writeln!(s, "    {} = {v};", q(k));
        }
        let _ = writeln!(s, "  }};");
    }
    let mut modules: Vec<&str> = Vec::new();
    if spec.kernel.sysctl.get("net.ipv4.tcp_congestion_control") == Some(&SysctlValue::Text("bbr".into())) {
        modules.push("tcp_bbr");
    }
    if !modules.is_empty() {
        let _ = writeln!(s, "  boot.kernelModules = {};", qlist(modules));
    }
    if spec.security.usb_storage == UsbStorage::Block {
        let _ = writeln!(s, "  boot.blacklistedKernelModules = [ \"usb_storage\" \"uas\" ];");
    }
    let gov = serde_json::to_value(spec.kernel.cpu_governor)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    let _ = writeln!(s, "  powerManagement.cpuFreqGovernor = {};", q(&gov));
    if let Some(bin) = spec.kernel.sched_ext.binary() {
        let _ = writeln!(s, "  services.scx = {{ enable = true; scheduler = {}; }};", q(bin));
    }
    if let Some(io) = spec.kernel.io_scheduler.kernel_name() {
        let rule = format!(
            "ACTION==\"add|change\", KERNEL==\"sd[a-z]*|nvme[0-9]*n[0-9]*|vd[a-z]*|mmcblk[0-9]*\", ENV{{DEVTYPE}}==\"disk\", ATTR{{queue/scheduler}}=\"{io}\"\n"
        );
        let _ = writeln!(s, "  services.udev.extraRules = {};", ind(&rule, "  "));
    }
    let _ = writeln!(s);

    // ---- drivers
    let _ = writeln!(s, "  # Drivers");
    let _ = writeln!(s, "  hardware.graphics.enable = true;");
    match spec.drivers.gpu {
        GpuDriver::Auto => {}
        GpuDriver::Nvidia | GpuDriver::NvidiaOpen => {
            let _ = writeln!(s, "  services.xserver.videoDrivers = [ \"nvidia\" ];");
            let _ = writeln!(
                s,
                "  hardware.nvidia = {{ open = {}; modesetting.enable = true; package = config.boot.kernelPackages.nvidiaPackages.stable; }};",
                spec.drivers.gpu == GpuDriver::NvidiaOpen
            );
        }
    }
    if spec.drivers.gaming_controllers {
        let _ = writeln!(s, "  hardware.xpadneo.enable = true;");
        let _ = writeln!(s, "  hardware.steam-hardware.enable = true;");
    }
    let _ = writeln!(s);

    // ---- desktop / session
    let _ = writeln!(s, "  # Desktop and session");
    let _ = writeln!(s, "  services.pipewire = {{ enable = true; alsa.enable = true; pulse.enable = true; }};");
    let _ = writeln!(s, "  security.rtkit.enable = true;");
    if kiosk {
        let _ = writeln!(s, "  users.users.kiosk = {{ isNormalUser = true; description = \"FluxOS kiosk\"; }};");
        let _ =
            writeln!(s, "  services.cage = {{ enable = true; user = \"kiosk\"; program = \"${{fluxosKiosk}}\"; }};");
        if spec.services.enabled.contains("flatpak") {
            // No desktop environment provides a portal in kiosk mode; Flatpak requires one.
            let _ = writeln!(
                s,
                "  xdg.portal = {{ enable = true; extraPortals = [ pkgs.xdg-desktop-portal-gtk ]; config.common.default = \"gtk\"; }};"
            );
        }
    } else {
        match spec.desktop.environment {
            DesktopEnv::Plasma => {
                let _ = writeln!(s, "  services.desktopManager.plasma6.enable = true;");
                let _ = writeln!(s, "  services.displayManager.sddm = {{ enable = true; wayland.enable = true; }};");
            }
            DesktopEnv::Gnome => {
                let _ = writeln!(s, "  services.desktopManager.gnome.enable = true;");
                let _ = writeln!(s, "  services.displayManager.gdm.enable = true;");
            }
            DesktopEnv::Hyprland => {
                let _ = writeln!(s, "  programs.hyprland.enable = true;");
                let _ = writeln!(s, "  services.displayManager.sddm = {{ enable = true; wayland.enable = true; }};");
            }
            DesktopEnv::Cosmic => {
                let _ = writeln!(s, "  services.desktopManager.cosmic.enable = true;");
                let _ = writeln!(s, "  services.displayManager.cosmic-greeter.enable = true;");
            }
        }
    }
    if spec.session.ephemeral_home {
        let _ = writeln!(
            s,
            "  users.users.guest = {{ isNormalUser = true; uid = 2001; description = \"Shared account (wiped on boot)\"; initialPassword = \"guest\"; }};"
        );
        let _ = writeln!(
            s,
            "  fileSystems.\"/home/guest\" = {{ device = \"tmpfs\"; fsType = \"tmpfs\"; options = [ \"size=25%\" \"mode=0700\" \"uid=2001\" \"gid=100\" ]; }};"
        );
    }
    let _ = writeln!(s);

    // ---- packages
    let _ = writeln!(s, "  # Packages");
    let mut sys_pkgs: Vec<String> = vec!["fluxosColorScheme".into(), "fluxosPlasmaLayout".into()];
    let mut module_lines: Vec<&str> = Vec::new();
    for id in &spec.packages.system {
        let Some(p) = catalog::package(id) else { continue };
        match p.nix {
            NixInstall::Pkg(attr) => sys_pkgs.push(pkg_expr(attr)),
            NixInstall::Expr(e) => sys_pkgs.push(format!("(pkgs.{e})")),
            NixInstall::Module(m) => module_lines.push(m),
            NixInstall::FlatpakOnly => {}
        }
    }
    if let Some(icon) = catalog::icon_theme(&spec.design.icon_theme) {
        sys_pkgs.push(pkg_expr(icon.nix));
    }
    if let Some(cursor) = catalog::cursor_theme(&spec.design.cursor_theme) {
        sys_pkgs.push(pkg_expr(cursor.nix));
    }
    if !kiosk && spec.desktop.environment == DesktopEnv::Gnome {
        sys_pkgs.push("pkgs.gnomeExtensions.appindicator".into());
        if spec.desktop.dock.enabled {
            sys_pkgs.push("pkgs.gnomeExtensions.dash-to-dock".into());
        }
        if spec.desktop.windows.blur {
            sys_pkgs.push("pkgs.gnomeExtensions.blur-my-shell".into());
        }
        if spec.desktop.windows.tiling {
            sys_pkgs.push("pkgs.gnomeExtensions.forge".into());
        }
    }
    if !kiosk && spec.desktop.environment == DesktopEnv::Plasma {
        if spec.desktop.windows.tiling {
            sys_pkgs.push("pkgs.kdePackages.krohnkite".into());
        } else if spec.desktop.windows.snapping {
            sys_pkgs.push("pkgs.kdePackages.kzones".into());
        }
    }
    sys_pkgs.dedup();
    let _ = writeln!(s, "  environment.systemPackages = [");
    for p in &sys_pkgs {
        let _ = writeln!(s, "    {p}");
    }
    let _ = writeln!(s, "  ];");
    let _ = writeln!(s, "  environment.pathsToLink = [ \"/share/color-schemes\" \"/share/plasma\" ];");
    for m in module_lines {
        let _ = writeln!(s, "  {m}");
    }
    let unfree = unfree_names(spec);
    if !unfree.is_empty() {
        let _ = writeln!(
            s,
            "  nixpkgs.config.allowUnfreePredicate = pkg: builtins.elem (lib.getName pkg) {};",
            qlist(unfree.iter().map(String::as_str))
        );
    }
    let _ = writeln!(s);

    // ---- services
    let _ = writeln!(s, "  # Services");
    for id in &spec.services.enabled {
        if let Some(svc) = catalog::service(id) {
            let _ = writeln!(s, "  {}", svc.nix);
        }
    }
    let flatpaks: Vec<&str> =
        spec.packages.flatpak.iter().filter_map(|id| catalog::package(id).and_then(|p| p.flatpak)).collect();
    if spec.services.enabled.contains("flatpak") {
        let script = format!(
            "set -eu\n\
             desired=\"{}\"\n\
             state=/var/lib/fluxos/flatpak-managed\n\
             mkdir -p /var/lib/fluxos\n\
             flatpak remote-add --system --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo\n\
             if [ -f \"$state\" ]; then\n\
             \x20 for app in $(cat \"$state\"); do\n\
             \x20   case \" $desired \" in *\" $app \"*) ;; *) flatpak uninstall --system --noninteractive \"$app\" || true ;; esac\n\
             \x20 done\n\
             fi\n\
             if [ -n \"$desired\" ]; then flatpak install --system --noninteractive --or-update flathub $desired; fi\n\
             echo \"$desired\" > \"$state\"\n",
            flatpaks.join(" ")
        );
        let _ = writeln!(s, "  systemd.services.fluxos-flatpak = {{");
        let _ = writeln!(s, "    description = \"FluxOS: sync Flathub apps\";");
        let _ = writeln!(s, "    wantedBy = [ \"multi-user.target\" ];");
        let _ = writeln!(s, "    wants = [ \"network-online.target\" ];");
        let _ = writeln!(s, "    after = [ \"network-online.target\" ];");
        let _ = writeln!(s, "    path = [ pkgs.flatpak ];");
        let _ = writeln!(s, "    serviceConfig = {{ Type = \"oneshot\"; RemainAfterExit = true; }};");
        let _ = writeln!(s, "    script = {};", ind(&script, "    "));
        let _ = writeln!(s, "  }};");
    }
    let _ = writeln!(s);

    // ---- security
    let sec = &spec.security;
    let _ = writeln!(s, "  # Security");
    let _ = writeln!(
        s,
        "  networking.firewall = {{ enable = true; allowedTCPPorts = {}; allowedUDPPorts = {}; }};",
        list(sec.firewall.allowed_tcp_ports.iter().map(u16::to_string)),
        list(sec.firewall.allowed_udp_ports.iter().map(u16::to_string))
    );
    let _ = writeln!(s, "  security.apparmor.enable = {};", sec.mac == MacPolicy::Apparmor);
    let _ = writeln!(s, "  security.audit.enable = true;");
    let _ = writeln!(s, "  security.auditd.enable = true;");
    let _ = writeln!(s);

    // ---- design
    let ty = &spec.design.tokens.typography;
    let mut font_pkgs: Vec<&str> = [&ty.font_sans, &ty.font_mono, &ty.font_display]
        .iter()
        .filter_map(|f| catalog::font(f).map(|e| e.nix))
        .collect();
    font_pkgs.push("noto-fonts-color-emoji");
    font_pkgs.sort();
    font_pkgs.dedup();
    let _ = writeln!(s, "  # Design ({})", spec.design.theme_name);
    let _ = writeln!(s, "  fonts.packages = {};", list(font_pkgs.iter().map(|p| pkg_expr(p))));
    let _ = writeln!(
        s,
        "  fonts.fontconfig.defaultFonts = {{ sansSerif = [ {} ]; monospace = [ {} ]; emoji = [ \"Noto Color Emoji\" ]; }};",
        q(&ty.font_sans),
        q(&ty.font_mono)
    );
    let etc = |s: &mut String, path: &str, content: &str| {
        let _ = writeln!(s, "  environment.etc.{}.text = {};", q(path), ind(content, "  "));
    };
    etc(&mut s, "fluxos/spec.json", &assets.spec_json);
    etc(&mut s, "fluxos/tokens.json", &assets.tokens_json);
    etc(&mut s, "fluxos/theme.css", &assets.theme_css);
    etc(&mut s, "fluxos/wallpaper.svg", &assets.wallpaper_svg);
    etc(&mut s, "fluxos/gtk-3.0.css", &assets.gtk_css);
    etc(&mut s, "fluxos/gtk-4.0.css", &assets.gtk_css);
    etc(&mut s, "xdg/gtk-3.0/settings.ini", &assets.gtk_settings);
    etc(&mut s, "xdg/gtk-4.0/settings.ini", &assets.gtk_settings);
    let _ = writeln!(s, "  environment.etc.\"fluxos/generation\".text = \"{}\\n\";", ctx.generation);
    let _ = writeln!(s, "  systemd.user.tmpfiles.rules = [");
    let _ = writeln!(s, "    \"L %h/.config/gtk-4.0/gtk.css - - - - /etc/fluxos/gtk-4.0.css\"");
    let _ = writeln!(s, "    \"L %h/.config/gtk-3.0/gtk.css - - - - /etc/fluxos/gtk-3.0.css\"");
    let _ = writeln!(s, "  ];");
    if !kiosk {
        match spec.desktop.environment {
            DesktopEnv::Plasma => {
                etc(&mut s, "xdg/kdeglobals", &assets.kdeglobals);
                etc(&mut s, "xdg/kwinrc", &assets.kwinrc);
                let cursor = format!("[Mouse]\ncursorTheme={}\n", spec.design.cursor_theme);
                etc(&mut s, "xdg/kcminputrc", &cursor);
                let lock = format!(
                    "[Daemon]\nAutolock={}\nTimeout={}\nLockOnResume=true\n",
                    sec.screen_lock.enabled, sec.screen_lock.idle_minutes
                );
                etc(&mut s, "xdg/kscreenlockerrc", &lock);
            }
            DesktopEnv::Gnome => {
                let _ = writeln!(s, "  programs.dconf.profiles.user.databases = [");
                let _ = writeln!(s, "    {{");
                let _ = writeln!(s, "      settings = {{");
                for (dir, entries) in gnome_dconf(spec) {
                    let _ = writeln!(s, "        {} = {{", q(&dir));
                    for (key, value) in entries {
                        let _ = writeln!(s, "          {key} = {};", dconf_value(&value));
                    }
                    let _ = writeln!(s, "        }};");
                }
                let _ = writeln!(s, "      }};");
                let _ = writeln!(s, "    }}");
                let _ = writeln!(s, "  ];");
            }
            DesktopEnv::Hyprland => {
                etc(&mut s, "xdg/hypr/fluxos.conf", &assets.hyprland);
            }
            DesktopEnv::Cosmic => {}
        }
    }
    let _ = writeln!(s);

    // ---- accessibility
    let _ = writeln!(s, "  # Accessibility");
    let _ = writeln!(s, "  services.orca.enable = {};", spec.accessibility.screen_reader);
    let _ = writeln!(s, "}}");
    s
}
