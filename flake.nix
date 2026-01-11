{
  description = "TeamSpeak 3 Music Bot";
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-25.11";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      fenix,
    }:
    let
      supportedSystems = [ "x86_64-linux" ];
      forEachSystem = nixpkgs.lib.genAttrs supportedSystems;
      overlayList = [ self.overlays.default ];
      pkgsBySystem = forEachSystem (
        system:
        import nixpkgs {
          inherit system;
          overlays = overlayList;
        }
      );
    in
    {
      overlays.default = final: prev: { pokebot = final.callPackage ./package.nix { }; };

      packages = forEachSystem (system: {
        pokebot =
          let
            inherit (fenix.packages.${system}.minimal) toolchain;
          in
          pkgsBySystem.callPackage ./package.nix {
            rustPlatform = pkgsBySystem.makeRustPlatform {
              cargo = toolchain;
              rustc = toolchain;
            };
          };
        default = self.packages.${system}.pokebot;
      });

      devShells = forEachSystem (system: {
        default = pkgsBySystem.${system}.callPackage ./shell.nix { };
      });

      nixosModules = import ./nixos-modules { overlays = overlayList; };
    };
}
