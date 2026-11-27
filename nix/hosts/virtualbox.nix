# The downloadable FluxOS VM: an OVA for VirtualBox that VMware Workstation,
# Player and Fusion can import too (File > Open). Build it with
#   nix build .#appliance-ova
#
# Hardware is chosen to exist in both hypervisors: BIOS boot from a SATA
# disk, an Intel e1000 NIC, HD audio and VMware-SVGA-compatible graphics
# (VirtualBox "VMSVGA"), driven by the in-kernel vmwgfx driver.
{ pkgs, modulesPath, ... }:

{
  imports = [ (modulesPath + "/virtualisation/virtualbox-image.nix") ];

  # fluxd rebuilds this same configuration inside the VM.
  services.fluxos.flake = "path:/etc/nixos#fluxos-appliance";

  image.baseName = "fluxos-appliance";
  # Virtual size; the root partition grows to fill it on first boot.
  virtualisation.diskSize = 32 * 1024;

  virtualbox = {
    vmName = "FluxOS";
    vmDerivationName = "fluxos-virtualbox";
    memorySize = 4096;
    params = {
      cpus = 2;
      ioapic = "on";
      vram = 128;
      graphicscontroller = "vmsvga";
      nictype1 = "82540EM";
      audiocontroller = "hda";
    };
    exportParams = [
      "--vsys" "0"
      "--product" "FluxOS"
      "--producturl" "https://github.com/Scafo745/FluxOs"
      "--vendor" "FluxOS"
      "--version" "0.2.0"
      "--description" "FluxOS: describe the computer you need. Log in as flux / flux. Change the password after the first boot."
    ];
  };

  # Guest tools userland built for the default kernel, with the in-kernel
  # vboxguest driver. Switching kernels (the gaming profile picks Zen, for
  # example) then never has to rebuild the guest additions.
  virtualisation.virtualbox.guest = {
    use3rdPartyModules = false;
    package = pkgs.linuxPackages.virtualboxGuestAdditions;
  };

  # The same image runs on VMware.
  virtualisation.vmware.guest.enable = true;
}
