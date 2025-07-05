{
  makeWrapper,
  pkg-config,
  cmake,
  glib,
  openssl,
  libopus,
  gst_all_1,
  lib,
  rustPlatform,
  yt-dlp,
}:
rustPlatform.buildRustPackage {
  pname = "pokebot";
  version = "0.3.0";
  cargoLock = {
    lockFile = ./Cargo.lock;
    outputHashes = {
      "ts-bookkeeping-0.1.0" = "sha256-luPHR729nPf1tDKeuZLPLNj/M3mSqFozm9odUlGCmgQ=";
    };
  };
  src = lib.cleanSource ./.;

  nativeBuildInputs = [makeWrapper pkg-config cmake];
  buildInputs =
    [
      glib
      openssl
      libopus
    ]
    ++ (with gst_all_1; [
      gstreamer
      gst-plugins-base
      gst-plugins-good
      gst-plugins-bad
      gst-plugins-ugly
    ]);

  postInstall = ''
    wrapProgram $out/bin/pokebot \
      --prefix GST_PLUGIN_SYSTEM_PATH_1_0 : "$GST_PLUGIN_SYSTEM_PATH_1_0" \
      --set PATH ${lib.makeBinPath [
      yt-dlp
    ]}
  '';

  meta = {
    description = "TeamSpeak 3 Music Bot";
    mainProgram = "pokebot";
    maintainers = with lib.maintainers; [jokler];
  };
}
