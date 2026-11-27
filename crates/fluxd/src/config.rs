use std::net::SocketAddr;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "fluxd", version, about = "FluxOS system daemon: the trust boundary between the AI and the machine")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Boot the daemon and serve the API.
    Serve(ServeArgs),
    /// Verify the hash chain of the audit log.
    AuditVerify {
        #[arg(long, env = "FLUX_STATE_DIR", default_value = "/var/lib/fluxos")]
        state_dir: PathBuf,
    },
    /// Render the artifacts of a built-in profile (or the default spec) to a directory.
    Render {
        #[arg(long, default_value = "default")]
        profile: String,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 1)]
        generation: u64,
    },
    /// Print the current one-time code for pairing a FluxOS UI.
    PairCode {
        #[arg(long, env = "FLUX_STATE_DIR", default_value = "/var/lib/fluxos")]
        state_dir: PathBuf,
    },
    /// Print the JSON schema of plans and specs.
    Schema,
    /// Print the tool registry.
    Tools,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum TargetKind {
    /// Materializes the system into the state directory. Safe anywhere.
    Sandbox,
    /// Writes /etc/nixos/fluxos/generated.nix and drives nixos-rebuild.
    Nixos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ModeArg {
    Personal,
    Fleet,
}

#[derive(Debug, Clone, Args)]
pub struct ServeArgs {
    #[arg(long, env = "FLUX_STATE_DIR", default_value = "/var/lib/fluxos")]
    pub state_dir: PathBuf,
    #[arg(long, env = "FLUX_LISTEN", default_value = "127.0.0.1:7700")]
    pub listen: SocketAddr,
    #[arg(long, env = "FLUX_TARGET", value_enum, default_value_t = TargetKind::Sandbox)]
    pub target: TargetKind,
    /// Overrides the mode in policy.json.
    #[arg(long, env = "FLUX_MODE", value_enum)]
    pub mode: Option<ModeArg>,
    /// Allowed browser origins for the web UI (comma separated).
    #[arg(
        long,
        env = "FLUX_CORS_ORIGINS",
        value_delimiter = ',',
        default_value = "http://localhost:5173,http://127.0.0.1:5173,http://localhost:3000"
    )]
    pub cors_origins: Vec<String>,
    /// Enables developer endpoints such as fault injection.
    #[arg(long, env = "FLUX_DEV")]
    pub dev: bool,
    /// Directory where the NixOS target writes generated.nix.
    #[arg(long, env = "FLUX_NIXOS_CONFIG_DIR", default_value = "/etc/nixos/fluxos")]
    pub nixos_config_dir: PathBuf,
    /// Flake reference for nixos-rebuild, e.g. `/etc/nixos#workstation`.
    #[arg(long, env = "FLUX_NIXOS_FLAKE")]
    pub nixos_flake: Option<String>,
    /// Log nixos-rebuild commands instead of running them.
    #[arg(long, env = "FLUX_NIXOS_DRY_RUN")]
    pub nixos_dry_run: bool,
    /// Serve the built web UI (ui/dist) at `/`.
    #[arg(long, env = "FLUX_UI_DIR")]
    pub ui_dir: Option<PathBuf>,
    /// Address of flux-agent as seen by the browser, told to paired UIs.
    #[arg(long, env = "FLUX_AGENT_PUBLIC_URL", default_value = "http://127.0.0.1:7701")]
    pub agent_url: String,
    /// Extra Host header names to accept (comma separated). Loopback names are
    /// always accepted; anything else is refused to stop DNS rebinding.
    #[arg(long, env = "FLUX_ALLOWED_HOSTS", value_delimiter = ',')]
    pub allowed_hosts: Vec<String>,
}
