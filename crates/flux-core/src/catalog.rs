//! Curated catalogs. Everything the agent can install or select comes from
//! here, so a hallucinated package name is rejected instead of reaching Nix.
//!
//! Nix attribute names were checked against nixpkgs `nixos-26.05`.

use serde::Serialize;

use crate::spec::{KernelVariant, SchedExt};

/// How a catalog package is installed on NixOS.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum NixInstall {
    /// `pkgs.<attr path>` added to `environment.systemPackages`.
    Pkg(&'static str),
    /// A parenthesized Nix expression added to `environment.systemPackages`.
    Expr(&'static str),
    /// NixOS module lines (for apps that need system integration).
    Module(&'static str),
    /// No Nix package; Flathub only.
    FlatpakOnly,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackageEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub nix: NixInstall,
    pub flatpak: Option<&'static str>,
    /// Package names (as seen by `lib.getName`) that need an unfree allowance.
    pub unfree: &'static [&'static str],
    pub tags: &'static [&'static str],
}

macro_rules! pkg {
    ($id:literal, $name:literal, $cat:literal, $desc:literal, $nix:expr, $flatpak:expr, $unfree:expr, $tags:expr) => {
        PackageEntry {
            id: $id,
            name: $name,
            category: $cat,
            description: $desc,
            nix: $nix,
            flatpak: $flatpak,
            unfree: $unfree,
            tags: $tags,
        }
    };
}

use NixInstall::{Expr, FlatpakOnly, Module, Pkg};

pub static PACKAGES: &[PackageEntry] = &[
    // Gaming
    pkg!(
        "steam",
        "Steam",
        "gaming",
        "Valve's game store and launcher with Proton",
        Module("programs.steam.enable = true;"),
        Some("com.valvesoftware.Steam"),
        &["steam", "steam-unwrapped", "steam-original", "steam-run"],
        &["games", "proton", "valve"]
    ),
    pkg!(
        "gamescope",
        "Gamescope",
        "gaming",
        "Micro-compositor for games (upscaling, frame limiting)",
        Module("programs.gamescope.enable = true;"),
        None,
        &[],
        &["games", "compositor"]
    ),
    pkg!(
        "lutris",
        "Lutris",
        "gaming",
        "Open gaming platform for Linux, Wine and emulators",
        Pkg("lutris"),
        Some("net.lutris.Lutris"),
        &[],
        &["games", "wine"]
    ),
    pkg!(
        "heroic",
        "Heroic Games Launcher",
        "gaming",
        "Epic, GOG and Amazon games launcher",
        Pkg("heroic"),
        Some("com.heroicgameslauncher.hgl"),
        &[],
        &["games", "epic", "gog"]
    ),
    pkg!(
        "protonup-qt",
        "ProtonUp-Qt",
        "gaming",
        "Install and manage Proton-GE and Wine-GE builds",
        Pkg("protonup-qt"),
        Some("net.davidotek.pupgui2"),
        &[],
        &["games", "proton"]
    ),
    pkg!(
        "mangohud",
        "MangoHud",
        "gaming",
        "In-game performance overlay (FPS, frametimes, temps)",
        Pkg("mangohud"),
        None,
        &[],
        &["games", "overlay", "fps"]
    ),
    pkg!(
        "goverlay",
        "GOverlay",
        "gaming",
        "GUI to configure MangoHud and vkBasalt",
        Pkg("goverlay"),
        None,
        &[],
        &["games", "overlay"]
    ),
    pkg!(
        "wine",
        "Wine",
        "gaming",
        "Windows compatibility layer (64/32-bit WoW64 build)",
        Pkg("wineWow64Packages.stable"),
        None,
        &[],
        &["windows", "compatibility"]
    ),
    pkg!(
        "winetricks",
        "Winetricks",
        "gaming",
        "Helper to install Windows runtimes into Wine prefixes",
        Pkg("winetricks"),
        None,
        &[],
        &["windows", "wine"]
    ),
    pkg!(
        "bottles",
        "Bottles",
        "gaming",
        "Run Windows software in managed Wine bottles",
        Pkg("bottles"),
        Some("com.usebottles.bottles"),
        &[],
        &["windows", "wine"]
    ),
    pkg!(
        "prismlauncher",
        "Prism Launcher",
        "gaming",
        "Minecraft launcher with mod support",
        Pkg("prismlauncher"),
        Some("org.prismlauncher.PrismLauncher"),
        &[],
        &["games", "minecraft"]
    ),
    pkg!(
        "discord",
        "Discord",
        "social",
        "Voice and text chat for communities",
        Pkg("discord"),
        Some("com.discordapp.Discord"),
        &["discord"],
        &["chat", "voice", "games"]
    ),
    pkg!(
        "obs-studio",
        "OBS Studio",
        "media",
        "Screen recording and live streaming",
        Pkg("obs-studio"),
        Some("com.obsproject.Studio"),
        &[],
        &["streaming", "recording"]
    ),
    // Everyday apps
    pkg!(
        "firefox",
        "Firefox",
        "web",
        "Mozilla web browser",
        Module("programs.firefox.enable = true;"),
        Some("org.mozilla.firefox"),
        &[],
        &["browser"]
    ),
    pkg!(
        "chromium",
        "Chromium",
        "web",
        "Open-source Chromium web browser",
        Pkg("chromium"),
        Some("org.chromium.Chromium"),
        &[],
        &["browser"]
    ),
    pkg!(
        "thunderbird",
        "Thunderbird",
        "office",
        "Email, calendar and contacts",
        Pkg("thunderbird"),
        Some("org.mozilla.Thunderbird"),
        &[],
        &["email", "calendar"]
    ),
    pkg!(
        "libreoffice",
        "LibreOffice",
        "office",
        "Office suite: documents, spreadsheets, slides",
        Pkg("libreoffice"),
        Some("org.libreoffice.LibreOffice"),
        &[],
        &["office", "documents", "spreadsheet"]
    ),
    pkg!(
        "vlc",
        "VLC",
        "media",
        "Media player for almost any format",
        Pkg("vlc"),
        Some("org.videolan.VLC"),
        &[],
        &["video", "music"]
    ),
    pkg!(
        "gimp",
        "GIMP",
        "creative",
        "Raster image editor",
        Pkg("gimp"),
        Some("org.gimp.GIMP"),
        &[],
        &["image", "photo"]
    ),
    pkg!(
        "inkscape",
        "Inkscape",
        "creative",
        "Vector graphics editor",
        Pkg("inkscape"),
        Some("org.inkscape.Inkscape"),
        &[],
        &["vector", "svg"]
    ),
    pkg!(
        "keepassxc",
        "KeePassXC",
        "security",
        "Offline password manager",
        Pkg("keepassxc"),
        Some("org.keepassxc.KeePassXC"),
        &[],
        &["passwords"]
    ),
    pkg!(
        "zoom",
        "Zoom",
        "meetings",
        "Zoom video meetings client",
        Pkg("zoom-us"),
        Some("us.zoom.Zoom"),
        &["zoom", "zoom-us"],
        &["video", "meetings", "classroom"]
    ),
    pkg!(
        "teams-for-linux",
        "Teams for Linux",
        "meetings",
        "Unofficial Microsoft Teams client",
        Pkg("teams-for-linux"),
        Some("com.github.IsmaelMartinez.teams_for_linux"),
        &[],
        &["video", "meetings", "classroom"]
    ),
    pkg!(
        "jitsi-meet",
        "Jitsi Meet",
        "meetings",
        "Open-source video conferencing client",
        Pkg("jitsi-meet-electron"),
        Some("org.jitsi.jitsi-meet"),
        &[],
        &["video", "meetings", "classroom"]
    ),
    pkg!(
        "element",
        "Element",
        "social",
        "Matrix chat client",
        Pkg("element-desktop"),
        Some("im.riot.Riot"),
        &[],
        &["chat", "matrix"]
    ),
    pkg!(
        "slack",
        "Slack",
        "social",
        "Team messaging",
        Pkg("slack"),
        Some("com.slack.Slack"),
        &["slack"],
        &["chat", "work"]
    ),
    pkg!(
        "remmina",
        "Remmina",
        "remote",
        "Remote desktop client (RDP, VNC, SPICE, SSH)",
        Pkg("remmina"),
        Some("org.remmina.Remmina"),
        &[],
        &["rdp", "vnc", "remote desktop", "vdi"]
    ),
    // Development
    pkg!(
        "vscodium",
        "VSCodium",
        "dev",
        "VS Code without Microsoft telemetry",
        Pkg("vscodium"),
        Some("com.vscodium.codium"),
        &[],
        &["editor", "ide", "coding"]
    ),
    pkg!(
        "vscode",
        "Visual Studio Code",
        "dev",
        "Microsoft's code editor",
        Pkg("vscode"),
        Some("com.visualstudio.code"),
        &["vscode"],
        &["editor", "ide", "coding"]
    ),
    pkg!("git", "Git", "dev", "Distributed version control", Pkg("git"), None, &[], &["vcs", "coding"]),
    pkg!("python3", "Python 3", "dev", "Python interpreter", Pkg("python3"), None, &[], &["coding", "python"]),
    pkg!("nodejs", "Node.js", "dev", "JavaScript runtime", Pkg("nodejs"), None, &[], &["coding", "javascript"]),
    pkg!("neovim", "Neovim", "dev", "Modal text editor", Pkg("neovim"), None, &[], &["editor", "terminal"]),
    pkg!(
        "thonny",
        "Thonny",
        "dev",
        "Beginner-friendly Python IDE",
        Pkg("thonny"),
        Some("org.thonny.Thonny"),
        &[],
        &["coding", "python", "classroom"]
    ),
    pkg!("jq", "jq", "dev", "Command-line JSON processor", Pkg("jq"), None, &[], &["json", "terminal"]),
    pkg!("ripgrep", "ripgrep", "dev", "Fast recursive search", Pkg("ripgrep"), None, &[], &["search", "terminal"]),
    // Education
    pkg!(
        "gcompris",
        "GCompris",
        "education",
        "Educational activities for children 2-10",
        Pkg("gcompris"),
        Some("org.kde.gcompris"),
        &[],
        &["kids", "classroom"]
    ),
    pkg!(
        "stellarium",
        "Stellarium",
        "education",
        "Planetarium: realistic 3D sky",
        Pkg("stellarium"),
        Some("org.stellarium.Stellarium"),
        &[],
        &["astronomy", "science", "classroom"]
    ),
    pkg!(
        "geogebra",
        "GeoGebra",
        "education",
        "Dynamic mathematics: geometry, algebra, graphing",
        Pkg("geogebra6"),
        Some("org.geogebra.GeoGebra"),
        &["geogebra"],
        &["math", "classroom"]
    ),
    pkg!(
        "anki",
        "Anki",
        "education",
        "Spaced-repetition flashcards",
        Pkg("anki"),
        Some("net.ankiweb.Anki"),
        &[],
        &["study", "flashcards"]
    ),
    // Networking and operations
    pkg!(
        "wireshark",
        "Wireshark",
        "network",
        "Packet capture and protocol analysis (GUI)",
        Module("programs.wireshark = { enable = true; package = pkgs.wireshark; };"),
        Some("org.wireshark.Wireshark"),
        &[],
        &["packets", "pcap", "noc", "telecom"]
    ),
    pkg!(
        "termshark",
        "termshark",
        "network",
        "Terminal UI for packet analysis",
        Pkg("termshark"),
        None,
        &[],
        &["packets", "pcap", "terminal", "telecom"]
    ),
    pkg!(
        "tcpdump",
        "tcpdump",
        "network",
        "Command-line packet capture",
        Pkg("tcpdump"),
        None,
        &[],
        &["packets", "pcap", "telecom"]
    ),
    pkg!(
        "nmap",
        "Nmap",
        "network",
        "Network discovery and port scanning (authorized networks only)",
        Pkg("nmap"),
        None,
        &[],
        &["scan", "noc"]
    ),
    pkg!(
        "mtr",
        "mtr",
        "network",
        "Traceroute and ping combined",
        Pkg("mtr"),
        None,
        &[],
        &["latency", "routing", "noc"]
    ),
    pkg!("iperf3", "iperf3", "network", "Network throughput testing", Pkg("iperf3"), None, &[], &["bandwidth", "noc"]),
    pkg!(
        "net-snmp",
        "Net-SNMP",
        "network",
        "SNMP clients (snmpget, snmpwalk) and tools",
        Pkg("net-snmp"),
        None,
        &[],
        &["snmp", "monitoring", "telecom"]
    ),
    pkg!(
        "netconf-client",
        "NETCONF client (ncclient)",
        "network",
        "Python with ncclient for NETCONF sessions",
        Expr("python3.withPackages (ps: [ ps.ncclient ])"),
        None,
        &[],
        &["netconf", "yang", "telecom"]
    ),
    pkg!(
        "sngrep",
        "sngrep",
        "network",
        "SIP message flow viewer",
        Pkg("sngrep"),
        None,
        &[],
        &["sip", "voip", "telecom"]
    ),
    pkg!(
        "sipp",
        "SIPp",
        "network",
        "SIP traffic generator and tester",
        Pkg("sipp"),
        None,
        &[],
        &["sip", "voip", "telecom"]
    ),
    pkg!(
        "baresip",
        "baresip",
        "network",
        "Modular SIP user agent (softphone)",
        Pkg("baresip"),
        None,
        &[],
        &["sip", "voip", "softphone", "telecom"]
    ),
    pkg!(
        "minicom",
        "minicom",
        "network",
        "Serial console for routers and switches",
        Pkg("minicom"),
        None,
        &[],
        &["serial", "console", "telecom"]
    ),
    pkg!("picocom", "picocom", "network", "Minimal serial terminal", Pkg("picocom"), None, &[], &["serial", "console"]),
    pkg!("tmux", "tmux", "network", "Terminal multiplexer", Pkg("tmux"), None, &[], &["terminal", "ssh", "noc"]),
    pkg!(
        "ansible",
        "Ansible",
        "network",
        "Agentless automation for network and servers",
        Pkg("ansible"),
        None,
        &[],
        &["automation", "noc"]
    ),
    pkg!("ipcalc", "ipcalc", "network", "IP subnet calculator", Pkg("ipcalc"), None, &[], &["subnet", "noc"]),
    pkg!(
        "dnsutils",
        "DNS utilities",
        "network",
        "dig, nslookup and friends",
        Pkg("dnsutils"),
        None,
        &[],
        &["dns", "noc"]
    ),
    pkg!("btop", "btop", "system", "Resource monitor", Pkg("btop"), None, &[], &["monitoring", "terminal"]),
    pkg!("htop", "htop", "system", "Interactive process viewer", Pkg("htop"), None, &[], &["monitoring", "terminal"]),
    pkg!(
        "iftop",
        "iftop",
        "network",
        "Bandwidth usage per connection",
        Pkg("iftop"),
        None,
        &[],
        &["bandwidth", "monitoring"]
    ),
    pkg!(
        "nethogs",
        "NetHogs",
        "network",
        "Bandwidth usage per process",
        Pkg("nethogs"),
        None,
        &[],
        &["bandwidth", "monitoring"]
    ),
    // Desktop helpers
    pkg!(
        "gnome-tweaks",
        "GNOME Tweaks",
        "desktop",
        "Advanced GNOME settings",
        Pkg("gnome-tweaks"),
        None,
        &[],
        &["gnome"]
    ),
    pkg!(
        "flatseal",
        "Flatseal",
        "desktop",
        "Manage Flatpak permissions",
        FlatpakOnly,
        Some("com.github.tchx84.Flatseal"),
        &[],
        &["flatpak", "permissions"]
    ),
];

#[derive(Debug, Clone, Serialize)]
pub struct ServiceEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// NixOS module lines.
    pub nix: &'static str,
    /// systemd units to health-check on a real target.
    pub units: &'static [&'static str],
    pub opens_tcp: &'static [u16],
    pub tags: &'static [&'static str],
}

pub static SERVICES: &[ServiceEntry] = &[
    ServiceEntry {
        id: "flatpak",
        name: "Flatpak",
        description: "Flathub app runtime for user-level apps",
        nix: "services.flatpak.enable = true;",
        units: &[],
        opens_tcp: &[],
        tags: &["apps"],
    },
    ServiceEntry {
        id: "fwupd",
        name: "Firmware updates",
        description: "LVFS firmware update daemon",
        nix: "services.fwupd.enable = true;",
        units: &[],
        opens_tcp: &[],
        tags: &["firmware"],
    },
    ServiceEntry {
        id: "ssh",
        name: "OpenSSH server",
        description: "Inbound SSH with keys only, root login disabled",
        nix: "services.openssh = { enable = true; settings = { PasswordAuthentication = false; KbdInteractiveAuthentication = false; PermitRootLogin = \"no\"; }; };",
        units: &["sshd.service"],
        opens_tcp: &[22],
        tags: &["remote", "ssh"],
    },
    ServiceEntry {
        id: "printing",
        name: "Printing (CUPS)",
        description: "Local and network printers",
        nix: "services.printing.enable = true;",
        units: &["cups.socket"],
        opens_tcp: &[],
        tags: &["printer", "office"],
    },
    ServiceEntry {
        id: "bluetooth",
        name: "Bluetooth",
        description: "Bluetooth stack (BlueZ)",
        nix: "hardware.bluetooth.enable = true;",
        units: &["bluetooth.service"],
        opens_tcp: &[],
        tags: &["wireless", "controllers", "headphones"],
    },
    ServiceEntry {
        id: "gamemode",
        name: "GameMode",
        description: "Feral GameMode: on-demand CPU/GPU performance boosts for games",
        nix: "programs.gamemode.enable = true;",
        units: &[],
        opens_tcp: &[],
        tags: &["games", "performance"],
    },
    ServiceEntry {
        id: "chrony",
        name: "Precise time sync (chrony)",
        description: "NTP client with fast convergence and accurate clock discipline",
        nix: "services.chrony.enable = true;",
        units: &["chronyd.service"],
        opens_tcp: &[],
        tags: &["time", "ntp", "finance", "telecom"],
    },
    ServiceEntry {
        id: "tailscale",
        name: "Tailscale",
        description: "WireGuard mesh VPN",
        nix: "services.tailscale.enable = true;",
        units: &["tailscaled.service"],
        opens_tcp: &[],
        tags: &["vpn", "remote"],
    },
    ServiceEntry {
        id: "docker",
        name: "Docker",
        description: "Docker container engine",
        nix: "virtualisation.docker.enable = true;",
        units: &["docker.service"],
        opens_tcp: &[],
        tags: &["containers", "dev"],
    },
    ServiceEntry {
        id: "podman",
        name: "Podman",
        description: "Daemonless container engine",
        nix: "virtualisation.podman.enable = true;",
        units: &[],
        opens_tcp: &[],
        tags: &["containers", "dev"],
    },
    ServiceEntry {
        id: "libvirtd",
        name: "Virtual machines (libvirt)",
        description: "KVM/QEMU virtualization for Windows-only tools",
        nix: "virtualisation.libvirtd.enable = true;\n  programs.virt-manager.enable = true;",
        units: &["libvirtd.service"],
        opens_tcp: &[],
        tags: &["vm", "windows", "compatibility"],
    },
    ServiceEntry {
        id: "avahi",
        name: "Local discovery (mDNS)",
        description: "Find printers and devices on the local network",
        nix: "services.avahi = { enable = true; nssmdns4 = true; };",
        units: &["avahi-daemon.service"],
        opens_tcp: &[],
        tags: &["network", "printer"],
    },
    ServiceEntry {
        id: "smartcard",
        name: "Smart card / badge login",
        description: "PC/SC daemon for badge and smart card readers",
        nix: "services.pcscd.enable = true;",
        units: &["pcscd.socket"],
        opens_tcp: &[],
        tags: &["badge", "healthcare", "login"],
    },
];

#[derive(Debug, Clone, Serialize)]
pub struct FontEntry {
    pub family: &'static str,
    pub nix: &'static str,
    pub kind: &'static str,
    pub license: &'static str,
    pub notes: &'static str,
}

pub static FONTS: &[FontEntry] = &[
    FontEntry {
        family: "Inter",
        nix: "inter",
        kind: "sans",
        license: "OFL-1.1",
        notes: "Neutral, highly legible UI sans",
    },
    FontEntry {
        family: "JetBrains Mono",
        nix: "jetbrains-mono",
        kind: "mono",
        license: "OFL-1.1",
        notes: "Code and terminals",
    },
    FontEntry {
        family: "IBM Plex Sans",
        nix: "ibm-plex",
        kind: "sans",
        license: "OFL-1.1",
        notes: "Technical, industrial feel",
    },
    FontEntry {
        family: "IBM Plex Mono",
        nix: "ibm-plex",
        kind: "mono",
        license: "OFL-1.1",
        notes: "Dense, readable mono for operations",
    },
    FontEntry {
        family: "Atkinson Hyperlegible Next",
        nix: "atkinson-hyperlegible-next",
        kind: "sans",
        license: "OFL-1.1",
        notes: "Designed for low-vision readers",
    },
    FontEntry {
        family: "Lexend",
        nix: "lexend",
        kind: "sans",
        license: "OFL-1.1",
        notes: "Built to improve reading fluency",
    },
    FontEntry {
        family: "Noto Sans",
        nix: "noto-fonts",
        kind: "sans",
        license: "OFL-1.1",
        notes: "Broad language coverage",
    },
    FontEntry {
        family: "Noto Sans Mono",
        nix: "noto-fonts",
        kind: "mono",
        license: "OFL-1.1",
        notes: "Broad language coverage",
    },
    FontEntry { family: "Fira Sans", nix: "fira-sans", kind: "sans", license: "OFL-1.1", notes: "Humanist sans" },
    FontEntry { family: "Fira Code", nix: "fira-code", kind: "mono", license: "OFL-1.1", notes: "Mono with ligatures" },
    FontEntry {
        family: "Source Sans 3",
        nix: "source-sans",
        kind: "sans",
        license: "OFL-1.1",
        notes: "Calm, professional sans",
    },
    FontEntry {
        family: "Open Sans",
        nix: "open-sans",
        kind: "sans",
        license: "OFL-1.1",
        notes: "Friendly, open forms",
    },
    FontEntry { family: "Roboto", nix: "roboto", kind: "sans", license: "Apache-2.0", notes: "Geometric-grotesque" },
    FontEntry { family: "Rubik", nix: "rubik", kind: "sans", license: "OFL-1.1", notes: "Rounded, bold, playful" },
    FontEntry {
        family: "Orbitron",
        nix: "orbitron",
        kind: "display",
        license: "OFL-1.1",
        notes: "Futuristic display face for gaming",
    },
];

#[derive(Debug, Clone, Serialize)]
pub struct ThemeEntry {
    pub name: &'static str,
    pub nix: &'static str,
    pub notes: &'static str,
}

pub static ICON_THEMES: &[ThemeEntry] = &[
    ThemeEntry { name: "Papirus", nix: "papirus-icon-theme", notes: "Flat, colorful" },
    ThemeEntry { name: "Papirus-Dark", nix: "papirus-icon-theme", notes: "Papirus for dark themes" },
    ThemeEntry { name: "Papirus-Light", nix: "papirus-icon-theme", notes: "Papirus for light themes" },
    ThemeEntry { name: "Tela", nix: "tela-icon-theme", notes: "Rounded, modern" },
    ThemeEntry { name: "Tela-dark", nix: "tela-icon-theme", notes: "Tela for dark themes" },
    ThemeEntry { name: "Tela-circle", nix: "tela-circle-icon-theme", notes: "Circular app icons" },
    ThemeEntry { name: "Tela-circle-dark", nix: "tela-circle-icon-theme", notes: "Circular icons for dark themes" },
    ThemeEntry { name: "Adwaita", nix: "adwaita-icon-theme", notes: "GNOME default" },
    ThemeEntry { name: "breeze", nix: "kdePackages.breeze-icons", notes: "KDE default" },
    ThemeEntry { name: "breeze-dark", nix: "kdePackages.breeze-icons", notes: "KDE default, dark" },
];

pub static CURSOR_THEMES: &[ThemeEntry] = &[
    ThemeEntry { name: "Bibata-Modern-Classic", nix: "bibata-cursors", notes: "Black, rounded" },
    ThemeEntry { name: "Bibata-Modern-Ice", nix: "bibata-cursors", notes: "White, rounded" },
    ThemeEntry { name: "phinger-cursors-dark", nix: "phinger-cursors", notes: "Minimal, dark" },
    ThemeEntry { name: "phinger-cursors-light", nix: "phinger-cursors", notes: "Minimal, light" },
    ThemeEntry { name: "Adwaita", nix: "adwaita-icon-theme", notes: "GNOME default" },
];

/// Plymouth themes. Built-ins ship with Plymouth; the rest come from
/// `adi1090x-plymouth-themes`.
pub static SPLASH_BUILTIN: &[&str] = &["bgrt", "spinner", "fade-in", "solar", "tribar", "spinfinity"];
pub static SPLASH_ADI1090X: &[&str] = &[
    "hexagon_dots",
    "rings",
    "circle_hud",
    "cuts",
    "lone",
    "colorful_loop",
    "angular",
    "cybernetic",
    "dna",
    "glowing",
    "hud_3",
    "loader_2",
    "owl",
    "seal",
    "sphere",
    "target",
    "tech_a",
    "square_hud",
    "black_hud",
    "connect",
];

pub fn splash_known(name: &str) -> bool {
    SPLASH_BUILTIN.contains(&name) || SPLASH_ADI1090X.contains(&name)
}

#[derive(Debug, Clone, Serialize)]
pub struct KernelEntry {
    pub variant: KernelVariant,
    pub label: &'static str,
    pub description: &'static str,
    /// Attribute under `pkgs` for `boot.kernelPackages`.
    pub nix_packages: &'static str,
    pub version_hint: &'static str,
    pub extra_params: &'static [&'static str],
    pub extra_sysctl: &'static [(&'static str, i64)],
    pub sched_ext: bool,
}

pub static KERNELS: &[KernelEntry] = &[
    KernelEntry {
        variant: KernelVariant::Lts,
        label: "LTS",
        description: "Long-term-support kernel, the safest default",
        nix_packages: "linuxPackages",
        version_hint: "6.18 LTS",
        extra_params: &[],
        extra_sysctl: &[],
        sched_ext: true,
    },
    KernelEntry {
        variant: KernelVariant::Latest,
        label: "Latest",
        description: "Newest stable kernel for recent hardware",
        nix_packages: "linuxPackages_latest",
        version_hint: "7.3",
        extra_params: &[],
        extra_sysctl: &[],
        sched_ext: true,
    },
    KernelEntry {
        variant: KernelVariant::LowLatency,
        label: "Low-latency (Zen)",
        description: "Zen kernel tuned for desktop and audio responsiveness",
        nix_packages: "linuxPackages_zen",
        version_hint: "zen",
        extra_params: &[],
        extra_sysctl: &[],
        sched_ext: true,
    },
    KernelEntry {
        variant: KernelVariant::Performance,
        label: "Performance (XanMod)",
        description: "XanMod kernel tuned for throughput and gaming",
        nix_packages: "linuxPackages_xanmod_latest",
        version_hint: "xanmod",
        extra_params: &[],
        extra_sysctl: &[],
        sched_ext: true,
    },
    KernelEntry {
        variant: KernelVariant::Hardened,
        label: "Hardened",
        description: "LTS kernel plus KSPP-style hardening parameters (nixpkgs 26.05 no longer ships linux_hardened)",
        nix_packages: "linuxPackages",
        version_hint: "6.18 LTS + hardening",
        extra_params: &[
            "slab_nomerge",
            "init_on_alloc=1",
            "init_on_free=1",
            "page_alloc.shuffle=1",
            "randomize_kstack_offset=on",
            "vsyscall=none",
            "debugfs=off",
        ],
        extra_sysctl: &[
            ("kernel.kptr_restrict", 2),
            ("kernel.dmesg_restrict", 1),
            ("kernel.unprivileged_bpf_disabled", 1),
            ("net.core.bpf_jit_harden", 2),
            ("kernel.yama.ptrace_scope", 1),
            ("kernel.kexec_load_disabled", 1),
        ],
        sched_ext: true,
    },
];

pub fn kernel(variant: KernelVariant) -> &'static KernelEntry {
    KERNELS.iter().find(|k| k.variant == variant).expect("every variant is cataloged")
}

#[derive(Debug, Clone, Serialize)]
pub struct SchedulerEntry {
    pub id: SchedExt,
    pub binary: Option<&'static str>,
    pub description: &'static str,
    /// `stable` or `experimental`; the PRD asks to validate maturity first.
    pub maturity: &'static str,
    pub good_for: &'static [&'static str],
}

pub static SCHEDULERS: &[SchedulerEntry] = &[
    SchedulerEntry {
        id: SchedExt::Off,
        binary: None,
        description: "Kernel default scheduler (EEVDF)",
        maturity: "stable",
        good_for: &["general"],
    },
    SchedulerEntry {
        id: SchedExt::Lavd,
        binary: Some("scx_lavd"),
        description: "Latency-criticality aware virtual deadline; built for gaming and interactive loads",
        maturity: "stable",
        good_for: &["gaming", "interactive", "low-latency"],
    },
    SchedulerEntry {
        id: SchedExt::Bpfland,
        binary: Some("scx_bpfland"),
        description: "Prioritizes interactive tasks for desktop responsiveness",
        maturity: "stable",
        good_for: &["desktop", "interactive"],
    },
    SchedulerEntry {
        id: SchedExt::Rusty,
        binary: Some("scx_rusty"),
        description: "Multi-domain load balancing for servers and mixed workloads",
        maturity: "stable",
        good_for: &["server", "throughput"],
    },
    SchedulerEntry {
        id: SchedExt::Flash,
        binary: Some("scx_flash"),
        description: "Fairness with low latency for audio and multimedia",
        maturity: "experimental",
        good_for: &["audio", "multimedia"],
    },
    SchedulerEntry {
        id: SchedExt::Cosmos,
        binary: Some("scx_cosmos"),
        description: "Lightweight, cache-aware scheduler",
        maturity: "experimental",
        good_for: &["general"],
    },
    SchedulerEntry {
        id: SchedExt::P2dq,
        binary: Some("scx_p2dq"),
        description: "Pick-two dispatch queues; general purpose",
        maturity: "experimental",
        good_for: &["general", "server"],
    },
];

pub fn scheduler(id: SchedExt) -> &'static SchedulerEntry {
    SCHEDULERS.iter().find(|s| s.id == id).expect("every scheduler is cataloged")
}

/// Value rule for an allowlisted sysctl key.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SysctlRule {
    Int {
        min: i64,
        max: i64,
    },
    Choice {
        values: &'static [&'static str],
    },
    /// Three space-separated integers "min default max" (tcp_rmem/tcp_wmem).
    Triple {
        min: i64,
        max: i64,
    },
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SysctlEntry {
    pub key: &'static str,
    pub rule: SysctlRule,
    pub description: &'static str,
}

const fn int(min: i64, max: i64) -> SysctlRule {
    SysctlRule::Int { min, max }
}

pub static SYSCTLS: &[SysctlEntry] = &[
    SysctlEntry {
        key: "vm.swappiness",
        rule: int(0, 200),
        description: "Preference for swapping vs. dropping page cache",
    },
    SysctlEntry {
        key: "vm.max_map_count",
        rule: int(65530, 2147483642),
        description: "Max memory map areas per process (some games need it high)",
    },
    SysctlEntry { key: "vm.dirty_ratio", rule: int(1, 100), description: "Dirty page percentage before writers block" },
    SysctlEntry {
        key: "vm.dirty_background_ratio",
        rule: int(1, 100),
        description: "Dirty page percentage before background writeback",
    },
    SysctlEntry {
        key: "vm.vfs_cache_pressure",
        rule: int(1, 1000),
        description: "Reclaim pressure on dentry/inode caches",
    },
    SysctlEntry {
        key: "kernel.split_lock_mitigate",
        rule: int(0, 1),
        description: "Split-lock slowdown mitigation (0 helps some games)",
    },
    SysctlEntry {
        key: "kernel.sched_autogroup_enabled",
        rule: int(0, 1),
        description: "Group tasks by session for desktop fairness",
    },
    SysctlEntry {
        key: "kernel.numa_balancing",
        rule: int(0, 1),
        description: "Automatic NUMA balancing (0 reduces jitter)",
    },
    SysctlEntry { key: "kernel.nmi_watchdog", rule: int(0, 1), description: "NMI watchdog (0 saves a little latency)" },
    SysctlEntry { key: "kernel.kptr_restrict", rule: int(0, 2), description: "Hide kernel pointers" },
    SysctlEntry { key: "kernel.dmesg_restrict", rule: int(0, 1), description: "Restrict dmesg to privileged users" },
    SysctlEntry { key: "kernel.unprivileged_bpf_disabled", rule: int(0, 2), description: "Disallow unprivileged eBPF" },
    SysctlEntry { key: "kernel.yama.ptrace_scope", rule: int(0, 3), description: "Restrict ptrace" },
    SysctlEntry { key: "kernel.kexec_load_disabled", rule: int(0, 1), description: "Disable kexec" },
    SysctlEntry { key: "net.core.bpf_jit_harden", rule: int(0, 2), description: "Harden the BPF JIT" },
    SysctlEntry {
        key: "fs.inotify.max_user_watches",
        rule: int(8192, 10485760),
        description: "File watches per user (IDEs, sync tools)",
    },
    SysctlEntry { key: "fs.file-max", rule: int(65536, 2147483647), description: "System-wide open file limit" },
    SysctlEntry {
        key: "net.core.default_qdisc",
        rule: SysctlRule::Choice { values: &["fq", "fq_codel", "cake", "pfifo_fast"] },
        description: "Default queueing discipline",
    },
    SysctlEntry {
        key: "net.ipv4.tcp_congestion_control",
        rule: SysctlRule::Choice { values: &["cubic", "bbr", "reno"] },
        description: "TCP congestion control algorithm",
    },
    SysctlEntry { key: "net.core.rmem_max", rule: int(212992, 1073741824), description: "Max socket receive buffer" },
    SysctlEntry { key: "net.core.wmem_max", rule: int(212992, 1073741824), description: "Max socket send buffer" },
    SysctlEntry {
        key: "net.core.rmem_default",
        rule: int(212992, 1073741824),
        description: "Default socket receive buffer",
    },
    SysctlEntry {
        key: "net.core.wmem_default",
        rule: int(212992, 1073741824),
        description: "Default socket send buffer",
    },
    SysctlEntry {
        key: "net.ipv4.tcp_rmem",
        rule: SysctlRule::Triple { min: 4096, max: 1073741824 },
        description: "TCP receive buffer min/default/max",
    },
    SysctlEntry {
        key: "net.ipv4.tcp_wmem",
        rule: SysctlRule::Triple { min: 4096, max: 1073741824 },
        description: "TCP send buffer min/default/max",
    },
    SysctlEntry {
        key: "net.core.netdev_max_backlog",
        rule: int(1000, 1000000),
        description: "Packets queued when the NIC outpaces the kernel",
    },
    SysctlEntry { key: "net.core.somaxconn", rule: int(128, 65535), description: "Listen backlog" },
    SysctlEntry {
        key: "net.core.busy_poll",
        rule: int(0, 200),
        description: "Busy-poll microseconds for low-latency sockets",
    },
    SysctlEntry {
        key: "net.core.busy_read",
        rule: int(0, 200),
        description: "Busy-read microseconds for low-latency sockets",
    },
    SysctlEntry { key: "net.ipv4.tcp_fastopen", rule: int(0, 3), description: "TCP Fast Open" },
    SysctlEntry { key: "net.ipv4.tcp_mtu_probing", rule: int(0, 2), description: "Path MTU probing" },
    SysctlEntry { key: "net.ipv4.tcp_slow_start_after_idle", rule: int(0, 1), description: "Reset cwnd after idle" },
    SysctlEntry {
        key: "net.ipv4.tcp_tw_reuse",
        rule: int(0, 2),
        description: "Reuse TIME-WAIT sockets for outbound connections",
    },
    SysctlEntry {
        key: "net.ipv4.ip_forward",
        rule: int(0, 1),
        description: "Route packets between interfaces (high risk)",
    },
    SysctlEntry { key: "net.ipv4.conf.all.rp_filter", rule: int(0, 2), description: "Reverse-path filtering" },
    SysctlEntry { key: "net.ipv4.icmp_echo_ignore_broadcasts", rule: int(0, 1), description: "Ignore broadcast pings" },
];

pub fn package(id: &str) -> Option<&'static PackageEntry> {
    PACKAGES.iter().find(|p| p.id == id)
}

pub fn service(id: &str) -> Option<&'static ServiceEntry> {
    SERVICES.iter().find(|s| s.id == id)
}

pub fn font(family: &str) -> Option<&'static FontEntry> {
    FONTS.iter().find(|f| f.family == family)
}

pub fn icon_theme(name: &str) -> Option<&'static ThemeEntry> {
    ICON_THEMES.iter().find(|t| t.name == name)
}

pub fn cursor_theme(name: &str) -> Option<&'static ThemeEntry> {
    CURSOR_THEMES.iter().find(|t| t.name == name)
}

pub fn sysctl(key: &str) -> Option<&'static SysctlEntry> {
    SYSCTLS.iter().find(|s| s.key == key)
}

/// Finds a package by id, Flathub id or case-insensitive display name.
pub fn resolve_package(query: &str) -> Option<&'static PackageEntry> {
    let q = query.trim();
    package(q).or_else(|| {
        PACKAGES.iter().find(|p| p.flatpak == Some(q) || p.name.eq_ignore_ascii_case(q) || p.id.eq_ignore_ascii_case(q))
    })
}

/// Closest catalog ids to `query`, for "did you mean" hints.
pub fn suggest<'a>(query: &str, candidates: impl Iterator<Item = &'a str>, n: usize) -> Vec<String> {
    let q = query.to_ascii_lowercase();
    let mut scored: Vec<(usize, &str)> = candidates
        .map(|c| {
            let cl = c.to_ascii_lowercase();
            let bonus = if cl.contains(&q) || q.contains(&cl) { 0 } else { 2 };
            (levenshtein(&q, &cl) + bonus, c)
        })
        .collect();
    scored.sort();
    scored.into_iter().take(n).map(|(_, c)| c.to_string()).collect()
}

pub fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Validates a sysctl assignment against the allowlist.
pub fn check_sysctl(key: &str, value: &crate::spec::SysctlValue) -> Result<(), String> {
    use crate::spec::SysctlValue;
    let Some(entry) = sysctl(key) else {
        return Err(format!("sysctl key {key:?} is not on the allowlist"));
    };
    match (entry.rule, value) {
        (SysctlRule::Int { min, max }, SysctlValue::Int(v)) => {
            if (min..=max).contains(v) {
                Ok(())
            } else {
                Err(format!("{key} must be between {min} and {max}, got {v}"))
            }
        }
        (SysctlRule::Int { min, max }, SysctlValue::Text(t)) => match t.trim().parse::<i64>() {
            Ok(v) if (min..=max).contains(&v) => Ok(()),
            _ => Err(format!("{key} must be an integer between {min} and {max}")),
        },
        (SysctlRule::Choice { values }, SysctlValue::Text(t)) => {
            if values.contains(&t.as_str()) {
                Ok(())
            } else {
                Err(format!("{key} must be one of {values:?}"))
            }
        }
        (SysctlRule::Choice { values }, SysctlValue::Int(_)) => Err(format!("{key} must be one of {values:?}")),
        (SysctlRule::Triple { min, max }, SysctlValue::Text(t)) => {
            let parts: Vec<i64> = t.split_whitespace().filter_map(|p| p.parse().ok()).collect();
            let shape_ok = parts.len() == 3 && t.split_whitespace().count() == 3;
            if shape_ok && parts.iter().all(|v| (min..=max).contains(v)) && parts[0] <= parts[1] && parts[1] <= parts[2]
            {
                Ok(())
            } else {
                Err(format!("{key} must be three ascending integers \"min default max\" within {min}..{max}"))
            }
        }
        (SysctlRule::Triple { .. }, SysctlValue::Int(_)) => Err(format!("{key} must be a string \"min default max\"")),
    }
}
