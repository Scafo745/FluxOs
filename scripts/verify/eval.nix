# Evaluates a FluxOS generated module as a complete NixOS system (no build).
# Used by scripts/verify/nix-eval.sh.
{ module, nixpkgs }:
let
  sys = import (nixpkgs + "/nixos") {
    system = "x86_64-linux";
    configuration = { ... }: {
      imports = [ module ];
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
}
