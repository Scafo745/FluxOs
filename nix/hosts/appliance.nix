# The FluxOS appliance: everything the downloadable VM image (virtualbox.nix)
# and the QEMU demo VM (vm.nix) share.
#
# First boot: the FluxOS flake is copied to /etc/nixos, fluxd adopts the spec
# the image was built from (generation 1), and the FluxOS shell starts on
# tty1, logged in as `flux` and already paired as the device owner. fluxd
# then applies new generations with nixos-rebuild against /etc/nixos.
{ config, lib, pkgs, fluxSrc, nixpkgsFlake, ... }:

{
  services.fluxos = {
    enable = true;
    target = "nixos";
    nixosConfigDir = "/etc/nixos/nix/hosts/fluxos";
    initialSpec = ./fluxos/spec.json;
    console = {
      user = "flux";
      # Single-user appliance with auto-login: the console user is the owner.
      autoPair = true;
      openAtLogin = true;
    };
  };

  systemd.services.fluxos-seed = {
    description = "Seed /etc/nixos with the FluxOS flake";
    wantedBy = [ "multi-user.target" ];
    before = [ "fluxd.service" ];
    requiredBy = [ "fluxd.service" ];
    serviceConfig = { Type = "oneshot"; RemainAfterExit = true; };
    script = ''
      if [ ! -e /etc/nixos/flake.nix ]; then
        mkdir -p /etc/nixos
        cp -rT ${fluxSrc} /etc/nixos
        chmod -R u+w /etc/nixos
      fi
    '';
  };

  users.users.flux = {
    isNormalUser = true;
    description = "FluxOS";
    extraGroups = [ "wheel" "networkmanager" "video" "audio" ];
    initialPassword = "flux";
  };
  # Desktops other than the FluxOS shell (after switching to Plasma, GNOME, ...) log in through a display manager.
  services.displayManager.autoLogin = { enable = true; user = "flux"; };

  nix.settings.experimental-features = [ "nix-command" "flakes" ];
  # nixos-rebuild evaluates the flake in /etc/nixos. Its locked nixpkgs input is
  # this store path, so evaluation needs no download.
  nix.registry.nixpkgs.flake = nixpkgsFlake;
  nix.nixPath = [ "nixpkgs=${nixpkgsFlake}" ];

  system.nixos.distroName = "FluxOS";
  services.getty.helpLine = lib.mkForce ''

    FluxOS: the FluxOS shell runs on tty1 (Ctrl+Alt+F1). Log in here as flux, password flux.
    Web UI: http://127.0.0.1:7700/   Pairing code: sudo fluxd pair-code
  '';
  documentation.nixos.enable = false;

  # Kernel messages also go to the first serial port, for automated boot tests.
  # The last console= is the primary one, so the splash and the shell stay on screen.
  boot.kernelParams = [ "console=ttyS0,115200" "console=tty1" "plymouth.ignore-serial-consoles" ];

  # End-to-end self-test for CI (scripts/ci/boot-test.sh). It only runs when the VM
  # is started with the SMBIOS credential fluxos.selftest, which VirtualBox and
  # VMware never set, and reports on the serial port.
  systemd.services.fluxos-selftest = {
    description = "FluxOS self-test (runs only with the fluxos.selftest credential)";
    wantedBy = [ "multi-user.target" ];
    after = [ "fluxd.service" "flux-agent.service" "network-online.target" ];
    wants = [ "network-online.target" ];
    unitConfig.ConditionCredential = "fluxos.selftest";
    path = with pkgs; [ curl jq coreutils gnugrep systemd ];
    serviceConfig = {
      Type = "oneshot";
      StandardOutput = "tty";
      StandardError = "tty";
      TTYPath = "/dev/ttyS0";
    };
    script = builtins.readFile ../../scripts/ci/selftest.sh;
  };

  system.stateVersion = "26.05";
}
