# QEMU demo machine for FluxOS. Boot it with `nix run .#vm`.
#
# On first boot the FluxOS flake is copied to /etc/nixos so fluxd can apply
# new generations with `nixos-rebuild --flake path:/etc/nixos#fluxos-vm`.
# Log in as `flux` / `flux` (change the password). The FluxOS UI opens after
# login, already paired.
{ config, lib, pkgs, modulesPath, fluxSrc, ... }:

let
  fluxd = "http://${config.services.fluxos.listen}";
  # Demo machine only: `flux` logs in automatically and is in wheel, so
  # handing it the pairing link grants nothing it could not already do.
  # On a real install a person pairs with `sudo fluxd pair-code`.
  openUi = pkgs.writeShellScript "fluxos-open-ui" ''
    url=${fluxd}/
    if [ -r /run/fluxos-demo/pair-url ]; then url=$(cat /run/fluxos-demo/pair-url); fi
    exec ${pkgs.xdg-utils}/bin/xdg-open "$url"
  '';
in
{
  imports = [ (modulesPath + "/virtualisation/qemu-vm.nix") ];

  systemd.services.fluxos-demo-pairing = {
    description = "Hand the FluxOS pairing link to the demo user";
    wantedBy = [ "multi-user.target" ];
    after = [ "fluxd.service" ];
    requires = [ "fluxd.service" ];
    path = [ pkgs.curl pkgs.coreutils ];
    serviceConfig = { Type = "oneshot"; RemainAfterExit = true; };
    # fluxd writes a fresh code before it starts listening, so wait for the API.
    script = ''
      for _ in $(seq 1 150); do curl -sf ${fluxd}/v1/health >/dev/null && break; sleep 0.2; done
      code=$(tr -d '\n-' < /var/lib/fluxos/secrets/pairing.code)
      install -d -m 0700 -o flux -g users /run/fluxos-demo
      printf '%s/#pair=%s\n' ${fluxd} "$code" > /run/fluxos-demo/pair-url
      chown flux:users /run/fluxos-demo/pair-url
      chmod 0400 /run/fluxos-demo/pair-url
    '';
  };

  environment.etc."xdg/autostart/fluxos.desktop".text = ''
    [Desktop Entry]
    Type=Application
    Name=FluxOS
    Exec=${openUi}
    NoDisplay=true
    X-KDE-autostart-phase=2
  '';

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
