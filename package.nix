{
  fetchurl,
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
  sqlite,
}: let
  swaggerUiZip = fetchurl {
    url = "https://github.com/swagger-api/swagger-ui/archive/refs/tags/v5.17.14.zip";
    hash = "sha256-SBJE0IEgl7Efuu73n3HZQrFxYX+cn5UU5jrL4T5xzNw="; # You'll need to get this hash
  };
in
  rustPlatform.buildRustPackage {
    pname = "pokebot";
    version = "0.3.0";
    cargoLock = {
      lockFile = ./Cargo.lock;
      outputHashes = {
        "ts-bookkeeping-0.1.0" = "sha256-0/l4tG3l6GnQlQkK0FOO9bm0ztEatidaIHMeKfQvgKE=";
      };
    };
    src = lib.cleanSource ./.;

    nativeBuildInputs = [makeWrapper pkg-config cmake];
    buildInputs =
      [
        glib
        openssl
        libopus
        sqlite
      ]
      ++ (with gst_all_1; [
        gstreamer
        gst-plugins-base
        gst-plugins-good
        gst-plugins-bad
        gst-plugins-ugly
      ]);

    SWAGGER_UI_ZIP = swaggerUiZip;

    # 2. Copy it to the local build directory and make it writable before Cargo runs
    preBuild = ''
      echo "Copying Swagger UI zip to build directory..."
      cp "$SWAGGER_UI_ZIP" ./swagger-ui-v5.17.14.zip
      chmod +w ./swagger-ui-v5.17.14.zip

      # 3. Point the crate to our local, writable copy
      export SWAGGER_UI_DOWNLOAD_URL="file://$(pwd)/swagger-ui-v5.17.14.zip"
    '';

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
