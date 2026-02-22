{
  description = "TeamSpeak 3 Music Bot";
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-25.11";
  };

  outputs = {
    self,
    nixpkgs,
  }: let
    supportedSystems = ["x86_64-linux"];
    forEachSystem = nixpkgs.lib.genAttrs supportedSystems;
    overlayList = [self.overlays.default];
    pkgsBySystem = forEachSystem (
      system:
        import nixpkgs {
          inherit system;
          overlays = overlayList;
        }
    );
  in {
    overlays.default = final: prev: {pokebot = final.callPackage ./package.nix {};};

    packages = forEachSystem (system: {
      pokebot =
        pkgsBySystem.${system}.callPackage ./package.nix {};
      default = self.packages.${system}.pokebot;
    });

    devShells = forEachSystem (system: {
      default = pkgsBySystem.${system}.callPackage ./shell.nix {};
    });

    nixosModules.default = import ./nixos-modules/pokebot-service.nix;
  };
}
