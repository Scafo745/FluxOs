{
  description = "FluxOS: describe the computer you need, and get it. AI-driven OS builder on NixOS.";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      fluxPackages = {
        services.fluxos.package = self.packages.${system}.fluxd;
        services.fluxos.agent.package = self.packages.${system}.flux-agent;
        services.fluxos.ui.package = self.packages.${system}.flux-ui;
      };
      # A FluxOS machine: the module, the packages, generation 1 and the host.
      fluxosSystem = host: nixpkgs.lib.nixosSystem {
        inherit system;
        specialArgs = { fluxSrc = self; nixpkgsFlake = nixpkgs; };
        modules = [
          self.nixosModules.default
          fluxPackages
          ./nix/hosts/fluxos/generated.nix
          ./nix/hosts/appliance.nix
          host
        ];
      };
    in
    {
      packages.${system} = {
        fluxd = pkgs.callPackage ./nix/fluxd.nix { };
        flux-agent = pkgs.callPackage ./nix/flux-agent.nix { };
        flux-ui = pkgs.callPackage ./nix/flux-ui.nix { };
        # The downloadable VM: import the .ova into VirtualBox or VMware.
        appliance-ova = self.nixosConfigurations.fluxos-appliance.config.system.build.virtualBoxOVA;
        default = self.packages.${system}.fluxd;
      };

      # Import into any NixOS configuration together with the generated module:
      #   imports = [ fluxos.nixosModules.default ./fluxos/generated.nix ];
      nixosModules.default = import ./nix/module.nix;

      # The appliance image (nix/hosts/virtualbox.nix). fluxd inside the VM
      # rebuilds this configuration from the copy of the flake in /etc/nixos.
      nixosConfigurations.fluxos-appliance = fluxosSystem ./nix/hosts/virtualbox.nix;

      # QEMU demo machine: `nix run .#vm` (or nixos-rebuild build-vm --flake .#fluxos-vm).
      nixosConfigurations.fluxos-vm = fluxosSystem ./nix/hosts/vm.nix;

      apps.${system}.vm = {
        type = "app";
        program = "${self.nixosConfigurations.fluxos-vm.config.system.build.vm}/bin/run-fluxos-vm";
      };

      devShells.${system}.default = pkgs.mkShell {
        packages = [ pkgs.cargo pkgs.rustc pkgs.clippy pkgs.rustfmt pkgs.nodejs_22 pkgs.jq pkgs.curl ];
      };
    };
}
