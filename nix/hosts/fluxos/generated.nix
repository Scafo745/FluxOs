# FluxOS generation 1: spec sha256:eef270178cd657fb0fbe72fc8ca7d89b4348a2101546ea41a123c98481ce2a65
# DO NOT EDIT. fluxd rewrites this file for every generation; change the system through FluxOS.
# Theme and desktop config files are not in here: they are live files under /run/fluxos/current, which fluxd
# points at the active generation, so design changes apply without a rebuild.
{ config, lib, pkgs, ... }:

{
  # Base
  networking.hostName = "fluxos";
  networking.networkmanager.enable = true;
  time.timeZone = "UTC";
  i18n.defaultLocale = "en_US.UTF-8";
  system.nixos.tags = [ "fluxos-gen-1" ];

  # Boot and kernel (LTS)
  boot.kernelPackages = pkgs.linuxPackages;
  boot.kernelParams = [ "quiet" "splash" "loglevel=3" "rd.udev.log_level=3" "udev.log_priority=3" ];
  boot.loader.timeout = 3;
  boot.consoleLogLevel = 3;
  boot.initrd.verbose = false;
  boot.plymouth = { enable = true; theme = "hexagon_dots"; themePackages = [ (pkgs.adi1090x-plymouth-themes.override { selected_themes = [ "hexagon_dots" ]; }) ]; };
  powerManagement.cpuFreqGovernor = "schedutil";

  # Drivers
  hardware.graphics.enable = true;

  # Desktop and session
  services.pipewire = { enable = true; alsa.enable = true; pulse.enable = true; };
  security.rtkit.enable = true;
  services.fluxos.shell.enable = true;
  xdg.portal = { enable = true; extraPortals = [ pkgs.xdg-desktop-portal-gtk ]; config.common.default = "gtk"; };

  # Packages
  environment.systemPackages = [
    pkgs.btop
    pkgs.git
    pkgs.papirus-icon-theme
    pkgs.bibata-cursors
  ];
  programs.firefox.enable = true;

  # Services
  services.flatpak.enable = true;
  services.fwupd.enable = true;
  systemd.services.fluxos-flatpak = {
    description = "FluxOS: sync Flathub apps";
    wantedBy = [ "multi-user.target" ];
    wants = [ "network-online.target" ];
    after = [ "network-online.target" ];
    path = [ pkgs.flatpak ];
    serviceConfig = { Type = "oneshot"; RemainAfterExit = true; };
    script = ''
      set -eu
      desired=""
      state=/var/lib/fluxos/flatpak-managed
      mkdir -p /var/lib/fluxos
      flatpak remote-add --system --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
      if [ -f "$state" ]; then
        for app in $(cat "$state"); do
          case " $desired " in *" $app "*) ;; *) flatpak uninstall --system --noninteractive "$app" || true ;; esac
        done
      fi
      if [ -n "$desired" ]; then flatpak install --system --noninteractive --or-update flathub $desired; fi
      echo "$desired" > "$state"
      '';
  };

  # Security
  networking.firewall = { enable = true; allowedTCPPorts = [ ]; allowedUDPPorts = [ ]; };
  security.apparmor.enable = true;
  security.audit.enable = true;
  security.auditd.enable = true;

  # Design: fonts, icons and cursors are packages; colors, motion and layout are live files.
  fonts.packages = [ pkgs.inter pkgs.jetbrains-mono pkgs.noto-fonts-color-emoji ];
  fonts.fontconfig.defaultFonts = { sansSerif = [ "Inter" ]; monospace = [ "JetBrains Mono" ]; emoji = [ "Noto Color Emoji" ]; };
  environment.etc."fluxos".source = "/run/fluxos/current/fluxos";
  environment.etc."xdg/gtk-3.0/settings.ini".source = "/run/fluxos/current/xdg/gtk-3.0/settings.ini";
  environment.etc."xdg/gtk-4.0/settings.ini".source = "/run/fluxos/current/xdg/gtk-4.0/settings.ini";
  environment.sessionVariables.XDG_DATA_DIRS = [ "/run/fluxos/current/share" ];
  systemd.user.tmpfiles.rules = [
    "L %h/.config/gtk-4.0/gtk.css - - - - /etc/fluxos/gtk-4.0.css"
    "L %h/.config/gtk-3.0/gtk.css - - - - /etc/fluxos/gtk-3.0.css"
  ];

  # Accessibility
  services.orca.enable = false;
}
