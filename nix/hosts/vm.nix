# QEMU demo machine for FluxOS. Boot it with `nix run .#vm`.
#
# On first boot the FluxOS flake is copied to /etc/nixos so fluxd can apply
# new generations with `nixos-rebuild --flake path:/etc/nixos#fluxos-vm`.
# Log in as `flux` / `flux` (change the password).
{ lib, modulesPath, fluxSrc, ... }:

{
  imports = [ (modulesPath + "/virtualisation/qemu-vm.nix") ];

  services.fluxos = {
    enable = true;
    target = "nixos";
    nixosConfigDir = "/etc/nixos/nix/hosts/fluxos";
    flake = "path:/etc/nixos#fluxos-vm";
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
    description = "FluxOS demo user";
    extraGroups = [ "wheel" "networkmanager" ];
    initialPassword = "flux";
  };
  services.displayManager.autoLogin = { enable = true; user = "flux"; };

  nix.settings.experimental-features = [ "nix-command" "flakes" ];

  virtualisation = {
    memorySize = 6144;
    cores = 4;
    diskSize = 30 * 1024;
    graphics = true;
  };

  system.stateVersion = "26.05";
}
