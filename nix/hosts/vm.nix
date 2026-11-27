# QEMU demo machine for FluxOS: `nix run .#vm` boots the appliance
# (appliance.nix) with the host's KVM, no image to import.
{ modulesPath, ... }:

{
  imports = [ (modulesPath + "/virtualisation/qemu-vm.nix") ];

  services.fluxos.flake = "path:/etc/nixos#fluxos-vm";

  virtualisation = {
    memorySize = 6144;
    cores = 4;
    diskSize = 30 * 1024;
    graphics = true;
    # VMware SVGA, the device VirtualBox and VMware users get.
    qemu.options = [ "-vga vmware" ];
  };
}
