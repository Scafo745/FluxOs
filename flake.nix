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
    in
    {
      packages.${system} = {
        fluxd = pkgs.callPackage ./nix/fluxd.nix { };
        flux-agent = pkgs.callPackage ./nix/flux-agent.nix { };
        flux-ui = pkgs.callPackage ./nix/flux-ui.nix { };
        default = self.packages.${system}.fluxd;
      };

      # Import into any NixOS configuration together with the generated module:
      #   imports = [ fluxos.nixosModules.default ./fluxos/generated.nix ];
      nixosModules.default = import ./nix/module.nix;

      # Bootable demo machine: `nix run .#vm` (or nixos-rebuild build-vm --flake .#fluxos-vm).
      # It boots generation 1 (the default profile); fluxd then rebuilds it in place.
      nixosConfigurations.fluxos-vm = nixpkgs.lib.nixosSystem {
        inherit system;
        specialArgs.fluxSrc = self;
        modules = [
          self.nixosModules.default
          fluxPackages
          ./nix/hosts/fluxos/generated.nix
          ./nix/hosts/vm.nix
        ];
      };

      apps.${system}.vm = {
        type = "app";
        program = "${self.nixosConfigurations.fluxos-vm.config.system.build.vm}/bin/run-fluxos-vm";
      };

      devShells.${system}.default = pkgs.mkShell {
        packages = [ pkgs.cargo pkgs.rustc pkgs.clippy pkgs.rustfmt pkgs.nodejs_22 pkgs.jq pkgs.curl ];
      };
    };
}
