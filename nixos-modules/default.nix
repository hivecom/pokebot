{overlays}: {
  onlyfan = import ./pokebot-service.nix;

  overlayNixpkgsForThisInstance = {pkgs, ...}: {
    nixpkgs = {
      inherit overlays;
    };
  };
}
