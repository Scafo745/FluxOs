# NixOS module: runs fluxd (privileged, drives nixos-rebuild, serves the web
# UI) and flux-agent (unprivileged, sandboxed) as systemd services, and the
# FluxOS shell (the web desktop) when generated.nix selects it.
#
# Usage (non-flake configuration.nix):
#   imports = [ /path/to/FluxOs/nix/module.nix ./fluxos/generated.nix ];
#   services.fluxos.enable = true;
#   services.fluxos.console.user = "alice";   # who uses the screen
#
# The UI is at http://127.0.0.1:7700/ ("FluxOS" in the app launcher). Pair a
# browser with the one-time code from `sudo fluxd pair-code`.
#
# fluxd rewrites `${nixosConfigDir}/generated.nix` when the configuration
# changes and runs nixos-rebuild, so the host configuration must import that
# file. Theme and layout files are live: fluxd keeps /run/fluxos/current
# pointed at the active generation, and generated.nix links them from there.
{ config, lib, pkgs, ... }:

let
  cfg = config.services.fluxos;
  inherit (lib) mkEnableOption mkIf mkMerge mkOption optional optionalString types concatStringsSep;

  # The UI is served by fluxd, so its origin is fluxd's address. The agent
  # (another port) must accept browser calls from it.
  listenPort = lib.last (lib.splitString ":" cfg.listen);
  uiUrl = "http://${cfg.listen}/";
  uiOrigins = lib.unique [ "http://${cfg.listen}" "http://127.0.0.1:${listenPort}" "http://localhost:${listenPort}" ];
  corsOrigins = concatStringsSep "," (lib.unique (cfg.corsOrigins ++ lib.optionals cfg.ui.enable uiOrigins));

  consoleUser = cfg.console.user;
  # Written by fluxos-console-pairing; readable by the console user only.
  pairUrlFile = "/run/fluxos-console/pair-url";

  # Opens the UI, with the current pairing link when one was handed over.
  openUi = pkgs.writeShellScript "fluxos-open-ui" ''
    url=${lib.escapeShellArg uiUrl}
    if [ -r ${pairUrlFile} ]; then url=$(${pkgs.coreutils}/bin/cat ${pairUrlFile}); fi
    exec ${pkgs.xdg-utils}/bin/xdg-open "$url"
  '';

  # The FluxOS shell: the web UI full screen, as the whole desktop.
  shellLauncher = pkgs.writeShellScript "fluxos-shell" ''
    url=${lib.escapeShellArg uiUrl}
    # fluxd serves the UI; without it the browser would only show an error page.
    for _ in $(${pkgs.coreutils}/bin/seq 1 240); do
      ${pkgs.curl}/bin/curl -sf -o /dev/null "$url" && break
      ${pkgs.coreutils}/bin/sleep 0.5
    done
    if [ -r ${pairUrlFile} ]; then url=$(${pkgs.coreutils}/bin/cat ${pairUrlFile}); fi
    profile="''${XDG_DATA_HOME:-$HOME/.local/share}/fluxos-shell"
    exec ${lib.getExe cfg.shell.browser} \
      --kiosk --app="$url" --user-data-dir="$profile" \
      --ozone-platform=wayland --no-first-run --no-default-browser-check \
      --noerrdialogs --disable-infobars --hide-crash-restore-bubble \
      --password-store=basic --disable-features=Translate,MediaRouter \
      --check-for-update-interval=31536000
  '';

  desktopItem = pkgs.makeDesktopItem {
    name = "fluxos";
    desktopName = "FluxOS";
    genericName = "AI system designer";
    comment = "Describe the computer you need";
    exec = "${openUi}";
    icon = "${cfg.ui.package}/fluxos.svg";
    categories = [ "System" "Settings" ];
    startupNotify = false;
  };
in
{
  options.services.fluxos = {
    enable = mkEnableOption "FluxOS system daemon (fluxd)";

    package = mkOption {
      type = types.package;
      default = pkgs.callPackage ./fluxd.nix { };
      description = "The fluxd package.";
    };

    target = mkOption {
      type = types.enum [ "nixos" "sandbox" ];
      default = "nixos";
      description = "`nixos` applies generations with nixos-rebuild; `sandbox` only materializes files under the state directory.";
    };

    mode = mkOption {
      type = types.enum [ "personal" "fleet" ];
      default = "personal";
      description = "In fleet mode, tier 2+ changes need an administrator token.";
    };

    listen = mkOption {
      type = types.str;
      default = "127.0.0.1:7700";
      description = "Address of the fluxd API. Keep it on loopback.";
    };

    nixosConfigDir = mkOption {
      type = types.str;
      default = "/etc/nixos/fluxos";
      description = "Directory where fluxd writes generated.nix.";
    };

    flake = mkOption {
      type = types.nullOr types.str;
      default = null;
      example = "path:/etc/nixos#workstation";
      description = "Flake reference passed to nixos-rebuild. Null uses the classic /etc/nixos/configuration.nix.";
    };

    dryRun = mkOption {
      type = types.bool;
      default = false;
      description = "Log nixos-rebuild commands instead of running them.";
    };

    dev = mkOption {
      type = types.bool;
      default = false;
      description = "Enable developer endpoints (fault injection).";
    };

    corsOrigins = mkOption {
      type = types.listOf types.str;
      default = [ "http://localhost:5173" "http://127.0.0.1:5173" ];
      description = "Extra browser origins allowed to call the APIs (the Vite dev server by default). The UI that fluxd serves is always allowed.";
    };

    allowedHosts = mkOption {
      type = types.listOf types.str;
      default = [ ];
      example = [ "fluxos.example.net" ];
      description = "Extra Host header names fluxd accepts. Loopback names always work; everything else is refused to stop DNS rebinding. Needed behind a reverse proxy that keeps the original Host.";
    };

    ui = {
      enable = mkOption {
        type = types.bool;
        default = true;
        description = "Serve the FluxOS web UI (boot screen, pairing, desktop shell) from fluxd at `http://<listen>/`, with a FluxOS launcher entry.";
      };
      package = mkOption {
        type = types.package;
        default = pkgs.callPackage ./flux-ui.nix { };
        defaultText = lib.literalExpression "pkgs.callPackage ./flux-ui.nix { }";
        description = "Static build of the web UI.";
      };
    };

    initialSpec = mkOption {
      type = types.nullOr types.path;
      default = null;
      example = lib.literalExpression "./fluxos/spec.json";
      description = "Spec this machine's generated.nix was rendered from (`fluxd render` writes it as etc/fluxos/spec.json). On first boot fluxd adopts it as generation 1 instead of the default profile, so nothing is rebuilt.";
    };

    console = {
      user = mkOption {
        type = types.nullOr types.str;
        default = null;
        example = "alice";
        description = "Account that uses this machine's screen. The FluxOS shell session runs as this user.";
      };
      autoPair = mkOption {
        type = types.bool;
        default = false;
        description = ''
          Pair the console user's FluxOS screen automatically: a root service hands the current
          pairing link to that user only. In personal mode a paired screen acts as the device
          owner, so enable this only where the console user is the owner, for example on a
          single-user machine that logs in automatically.
        '';
      };
      openAtLogin = mkOption {
        type = types.bool;
        default = false;
        description = "Open the FluxOS UI in the default browser when the console user logs in to a desktop other than the FluxOS shell.";
      };
    };

    shell = {
      enable = mkOption {
        type = types.bool;
        default = false;
        description = "Run the FluxOS shell, the web desktop, as the graphical session: cage and a full-screen browser on tty1, logged in as `console.user`. generated.nix turns this on when the spec's desktop environment is `fluxos`.";
      };
      browser = lib.mkPackageOption pkgs "chromium" { };
    };

    agent = {
      enable = mkOption {
        type = types.bool;
        default = true;
        description = "Run the FluxOS AI orchestrator.";
      };
      package = mkOption {
        type = types.package;
        default = pkgs.callPackage ./flux-agent.nix { };
        description = "The flux-agent package.";
      };
      port = mkOption {
        type = types.port;
        default = 7701;
      };
      providers = mkOption {
        type = types.str;
        default = "anthropic,openai,ollama";
        description = "Failover order of AI providers. Providers without credentials are skipped.";
      };
      routing = mkOption {
        type = types.enum [ "auto" "local" "llm" ];
        default = "auto";
        description = "`auto` uses the local rule engine for common requests and an LLM for novel ones.";
      };
      environmentFile = mkOption {
        type = types.nullOr types.path;
        default = null;
        example = "/run/secrets/flux-agent.env";
        description = "File with ANTHROPIC_API_KEY=..., OPENAI_API_KEY=..., OLLAMA_HOST=... (kept out of the Nix store).";
      };
    };
  };

  config = mkIf cfg.enable (mkMerge [ {
    environment.systemPackages = [ cfg.package ] ++ lib.optional cfg.ui.enable desktopItem;

    # Live theme files (see crates/fluxd/src/targets/nixos.rs). On a tmpfs: fluxd republishes them at boot.
    systemd.tmpfiles.rules = [ "d /run/fluxos 0755 root root -" ];
    environment.etc."fluxos-image/spec.json" = mkIf (cfg.initialSpec != null) { source = cfg.initialSpec; };

    systemd.services.fluxd = {
      description = "FluxOS system daemon";
      wantedBy = [ "multi-user.target" ];
      after = [ "network.target" ];
      # Sessions read the live theme files fluxd publishes at boot.
      before = [ "display-manager.service" "cage-tty1.service" ];
      # nixos-rebuild, nix and systemctl are the only tools fluxd runs.
      path = [ config.system.build.nixos-rebuild config.nix.package pkgs.git pkgs.systemd pkgs.iproute2 pkgs.coreutils ];
      environment = {
        RUST_LOG = "info,tower_http=warn";
        FLUX_CORS_ORIGINS = corsOrigins;
        # nix keeps its evaluation and fetcher caches here.
        HOME = "/root";
      };
      serviceConfig = {
        # `nixos-rebuild build` leaves its ./result link here.
        WorkingDirectory = "/var/lib/fluxos";
        ExecStart =
          "${cfg.package}/bin/fluxd serve --state-dir /var/lib/fluxos --listen ${cfg.listen}"
          + " --target ${cfg.target} --mode ${cfg.mode} --nixos-config-dir ${cfg.nixosConfigDir}"
          + " --agent-url http://127.0.0.1:${toString cfg.agent.port}"
          + optionalString cfg.ui.enable " --ui-dir ${cfg.ui.package}"
          + optionalString (cfg.allowedHosts != [ ]) " --allowed-hosts ${concatStringsSep "," cfg.allowedHosts}"
          + optionalString (cfg.flake != null) " --nixos-flake ${cfg.flake}"
          + optionalString cfg.dryRun " --nixos-dry-run"
          + optionalString cfg.dev " --dev";
        StateDirectory = "fluxos";
        StateDirectoryMode = "0700";
        Restart = "on-failure";
        RestartSec = 2;
        # fluxd must stay privileged to rebuild the system; limit everything else.
        PrivateTmp = true;
        LockPersonality = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        UMask = "0077";
      };
    };

    systemd.services.flux-agent = mkIf cfg.agent.enable {
      description = "FluxOS AI orchestrator";
      wantedBy = [ "multi-user.target" ];
      after = [ "fluxd.service" "network-online.target" ];
      wants = [ "network-online.target" ];
      requires = [ "fluxd.service" ];
      environment = {
        FLUXD_URL = "http://${cfg.listen}";
        FLUX_AGENT_HOST = "127.0.0.1";
        FLUX_AGENT_PORT = toString cfg.agent.port;
        FLUX_AGENT_TOKEN_FILE = "%d/agent-token";
        FLUX_INTENT_TOKEN_FILE = "%d/intent-token";
        FLUX_AI_PROVIDERS = cfg.agent.providers;
        FLUX_AI_ROUTING = cfg.agent.routing;
        FLUX_CORS_ORIGINS = corsOrigins;
      };
      serviceConfig = {
        ExecStart = "${cfg.agent.package}/bin/flux-agent";
        # Tokens are handed over as credentials; the agent never reads fluxd's state directory.
        LoadCredential = [
          "agent-token:/var/lib/fluxos/secrets/agent.token"
          "intent-token:/var/lib/fluxos/secrets/intent.token"
        ];
        EnvironmentFile = lib.optional (cfg.agent.environmentFile != null) cfg.agent.environmentFile;
        DynamicUser = true;
        Restart = "on-failure";
        RestartSec = 2;
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
        PrivateDevices = true;
        NoNewPrivileges = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectKernelLogs = true;
        ProtectControlGroups = true;
        ProtectClock = true;
        ProtectHostname = true;
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        LockPersonality = true;
        RestrictAddressFamilies = [ "AF_INET" "AF_INET6" "AF_UNIX" ];
        CapabilityBoundingSet = "";
        SystemCallArchitectures = "native";
        SystemCallFilter = [ "@system-service" ];
        UMask = "0077";
      };
    };

    assertions = [
      {
        assertion = lib.hasPrefix "127.0.0.1:" cfg.listen || lib.hasPrefix "[::1]:" cfg.listen || lib.hasPrefix "localhost:" cfg.listen;
        message = "services.fluxos.listen must stay on loopback; put a reverse proxy with TLS and auth in front for remote access.";
      }
      {
        assertion = (cfg.shell.enable || cfg.console.autoPair || cfg.console.openAtLogin) -> consoleUser != null;
        message = "services.fluxos: the FluxOS shell and console pairing need services.fluxos.console.user.";
      }
      {
        assertion = cfg.shell.enable -> cfg.ui.enable;
        message = "services.fluxos.shell needs services.fluxos.ui.enable: fluxd serves the shell.";
      }
    ];

    warnings = lib.optional (cfg.target == "nixos" && cfg.dryRun) "services.fluxos: dryRun is on, generations will not be activated.";
  }

  (mkIf (cfg.shell.enable && consoleUser != null) {
    services.cage = {
      enable = true;
      user = consoleUser;
      program = shellLauncher;
      extraArguments = [ "-d" ];
    };
    systemd.services.cage-tty1 = {
      wants = [ "fluxd.service" ] ++ optional cfg.console.autoPair "fluxos-console-pairing.service";
      after = [ "fluxd.service" ] ++ optional cfg.console.autoPair "fluxos-console-pairing.service";
      # The shell comes back if the browser is closed or crashes.
      serviceConfig = { Restart = "always"; RestartSec = 2; };
    };
    users.users.${consoleUser}.isNormalUser = lib.mkDefault true;
  })

  (mkIf (cfg.console.autoPair && consoleUser != null) {
    # fluxd writes a new one-time code at start and after every pairing; keep the link current.
    systemd.services.fluxos-console-pairing = {
      description = "Hand the FluxOS pairing link to the console user";
      wantedBy = [ "multi-user.target" ];
      after = [ "fluxd.service" ];
      requires = [ "fluxd.service" ];
      path = [ pkgs.coreutils ];
      serviceConfig.Type = "oneshot";
      script = ''
        code=/var/lib/fluxos/secrets/pairing.code
        for _ in $(seq 1 100); do [ -s "$code" ] && break; sleep 0.2; done
        [ -s "$code" ] || exit 0
        dir=$(dirname ${pairUrlFile})
        install -d -m 0700 -o ${consoleUser} -g ${config.users.users.${consoleUser}.group} "$dir"
        umask 077
        printf '%s#pair=%s\n' ${lib.escapeShellArg uiUrl} "$(tr -d '\n-' < "$code")" > "$dir/.pair-url"
        chown ${consoleUser} "$dir/.pair-url"
        mv "$dir/.pair-url" ${pairUrlFile}
      '';
    };
    systemd.paths.fluxos-console-pairing = {
      wantedBy = [ "multi-user.target" ];
      pathConfig.PathChanged = "/var/lib/fluxos/secrets/pairing.code";
    };
  })

  (mkIf (cfg.console.openAtLogin && consoleUser != null && cfg.ui.enable) {
    environment.etc."xdg/autostart/fluxos.desktop".text = ''
      [Desktop Entry]
      Type=Application
      Name=FluxOS
      Exec=${openUi}
      NoDisplay=true
      X-KDE-autostart-phase=2
    '';
  })
  ]);
}
