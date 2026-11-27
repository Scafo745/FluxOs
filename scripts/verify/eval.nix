# Evaluates a FluxOS generated module as a complete NixOS system (no build),
# together with the FluxOS NixOS module it relies on (the FluxOS shell, live
# theme files). Used by scripts/verify/nix-eval.sh.
{ module, nixpkgs, root }:
let
  sys = import (nixpkgs + "/nixos") {
    system = "x86_64-linux";
    configuration = { ... }: {
      imports = [ (root + "/nix/module.nix") module ];
      services.fluxos = { enable = true; console.user = "flux"; };
      boot.loader.systemd-boot.enable = true;
      boot.loader.efi.canTouchEfiVariables = true;
      fileSystems."/" = { device = "/dev/disk/by-label/nixos"; fsType = "ext4"; };
      system.stateVersion = "26.05";
    };
  };
in
{
  drv = sys.config.system.build.toplevel.drvPath;
  warnings = sys.config.warnings;
  kernel = sys.config.boot.kernelPackages.kernel.version;
  scx = if sys.config.services.scx.enable then sys.config.services.scx.scheduler else "off";
  session =
    if sys.config.services.cage.enable then "cage (${if sys.config.services.fluxos.shell.enable then "FluxOS shell" else "kiosk"})"
    else if sys.config.services.displayManager.sddm.enable then "sddm"
    else if sys.config.services.displayManager.gdm.enable then "gdm"
    else "other";
}
