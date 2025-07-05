{
  description = "TeamSpeak 3 Music Bot";
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-25.05";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    fenix,
  }: let
    supportedSystems = ["x86_64-linux"];
    forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    overlayList = [self.overlays.default];
    pkgsFor = forAllSystems (system:
      import nixpkgs {
        inherit system;
        overlays = overlayList;
      });
    # rustPlatform = forAllSystems (system: { let
    #   toolchain = fenix.packages.${system}.stable.toolchain;
    # in
    #   pkgsFor.${system}.makeRustPlatform {
    #     cargo = toolchain;
    #     rustc = toolchain;
    #   };
    # });
  in {
    overlays.default = final: prev: {
      pokebot = final.callPackage ./package.nix {
        # FIXME: use variable for system
        rustPlatform = let
          toolchain = fenix.packages."x86_64-linux".stable.toolchain;
        in
          pkgsFor."x86_64-linux".makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          };
      };
    };

    packages = forAllSystems (system: {
      pokebot = pkgsFor.${system}.pokebot;
      default = pkgsFor.${system}.pokebot;
    });
    devShells = forAllSystems (system: {
      default = pkgsFor.${system}.callPackage ./shell.nix {};
    });

    nixosModules = import ./nixos-modules {overlays = overlayList;};
  };
}
