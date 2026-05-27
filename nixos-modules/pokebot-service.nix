{
  modulesPath,
  config,
  pkgs,
  lib ? pkgs.lib,
  ...
}:
with lib; let
  cfg = config.services.pokebot;
  defaultUser = "pokebot";
  format = pkgs.formats.toml {};
  configFile =
    if cfg.configFile != null
    then cfg.configFile
    else
      format.generate "pokebot.toml" {
        address = cfg.teamspeakAddress;
        inherit (cfg.main) channel;
        music_root = cfg.musicRoot;
        verbose = cfg.verbosity;
        volume = cfg.music.defaultVolume;
        webserver_enable = cfg.webserver.enable;
        inherit (cfg.webserver) domain;
        bind_address = cfg.webserver.bindAddress;
        id = cfg.main.identity;
        master_name = cfg.main.name;
        inherit (cfg.music) names;
        ids = cfg.music.identities;
      };
in {
  ###### interface
  options = {
    services.pokebot = {
      enable = mkOption {
        type = types.bool;
        default = false;
        description = ''
          Whether to run pokebot.
        '';
      };
      package = mkOption {
        type = types.package;
        default = pkgs.callPackage ../package.nix {};
        description = "Pokebot package";
      };

      logLevel = mkOption {
        type = types.str;
        default = "debug";
        description = ''
          Rust log level: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#example-syntax
        '';
      };

      user = mkOption {
        default = defaultUser;
        example = "john";
        type = types.str;
        description = ''
          The name of an existing user account to use to own the pokebot server
          process. If not specified, a default user will be created.
        '';
      };

      group = mkOption {
        default = defaultUser;
        example = "users";
        type = types.str;
        description = ''
          Group to own the pokebot process.
        '';
      };

      configFile = mkOption {
        type = types.nullOr types.path;
        default = null;
        description = ''
          Config file path for Pokebot.
          If this option is defined, the rest of the configuration will be ignored.
        '';
      };

      teamspeakAddress = mkOption {
        type = types.str;
        default = "localhost";
        description = ''
          Address of the teamspeak the bot is supposed to connect to.
        '';
      };
      musicRoot = mkOption {
        type = types.str;
        description = ''
          Location to look for music in.
        '';
      };
      verbosity = mkOption {
        type = types.int;
        default = 0;
        description = ''
          Verbosity of teamspeak connection logs.
        '';
      };
      webserver = {
        enable = mkOption {
          type = types.bool;
          default = false;
          description = ''
            Whether to enable the webserver.
          '';
        };
        bindAddress = mkOption {
          type = types.str;
          default = "0.0.0.0:7992";
          description = ''
            Address to bind the webserver to.
          '';
        };
        domain = mkOption {
          type = types.str;
          description = ''
            Domain to use within the webserver.
          '';
        };
        databasePath = mkOption {
          type = types.str;
          default = "pokebot.db";
          description = ''
            Path to store the SQlite datbase at.
          '';
        };
        dataDir = mkOption {
          type = types.path;
          default = "/var/lib/pokebot";
          description = ''
            Where pokebot should store its files.
          '';
        };

        nginx = {
          enable = mkOption {
            type = types.bool;
            default = false;
            description = ''
              Whether to enable nginx virtual host management.
              Further nginx configuration can be done by adapting <literal>services.nginx.virtualHosts.&lt;name&gt;</literal>.
              See <xref linkend="opt-services.nginx.virtualHosts"/> for further information.
            '';
          };
          virtualHost = mkOption {
            type = types.submodule (
              recursiveUpdate (import (modulesPath + "/services/web-servers/nginx/vhost-options.nix") {
                inherit config lib;
              }) {}
            );
            example = literalExpression ''
              {
                serverName = "pokebot.example.org";
                forceSSL = true;
                enableACME = true;
              }
            '';
            description = ''
              Nginx configuration can be done by adapting `services.nginx.virtualHosts.<name>`.
              See [](#opt-services.nginx.virtualHosts) for further information.
            '';
          };
        };
      };
      main = {
        name = mkOption {
          type = types.str;
          description = ''
            Name of the main bot.
          '';
        };
        channel = mkOption {
          type = types.str;
          description = ''
            Default channel to connect to.
          '';
        };
        identity = mkOption {
          type = types.attrsOf types.anything;
          description = ''
            Identity of the main bot.
          '';
        };
      };
      music = {
        names = mkOption {
          type = types.listOf types.str;
          description = ''
            Names of the music bots.
          '';
        };
        defaultVolume = mkOption {
          type = types.float;
          default = 0.3;
          description = ''
            Default volume of music bots.
          '';
        };
        identities = mkOption {
          type = types.listOf (types.attrsOf types.anything);
          description = ''
            Identities of the music bots.
          '';
        };
      };
    };
  };

  ###### implementation

  config = mkIf cfg.enable {
    systemd.services.pokebot = {
      wantedBy = ["multi-user.target"];
      after = ["network-online.target"];
      wants = ["network-online.target"];
      description = "TeamSpeak 3 Music Bot";
      environment = {
        RUST_LOG = cfg.logLevel;
        WEB_ROOT = "${cfg.package}/share/pokebot";
        DATABASE_URL = cfg.webserver.databasePath;
      };
      serviceConfig = {
        LoadCredential = "config.toml:${configFile}";
        ExecStart = "${getExe cfg.package} $\{CREDENTIALS_DIRECTORY\}/config.toml";
        Restart = "always";
        RestartSec = 30;
        WorkingDirectory = cfg.webserver.dataDir;

        User = cfg.user;
        Group = cfg.group;
        LockPersonality = true;
        ProtectSystem = true;
        ProtectClock = true;
        ProtectControlGroups = true;
        ProtectHome = true;
        ProtectHostname = true;
        ProtectKernelLogs = true;
        ProtectKernelModules = true;
        ProtectKernelTunables = true;
        ProtectProc = "invisible";
        RemoveIPC = true;
        RestrictAddressFamilies = [];
        RestrictNamespaces = true;
        RestrictRealtime = true;
        NoNewPrivileges = true;
        PrivateDevices = true;
        PrivateTmp = true;
      };
    };

    users.users = optionalAttrs (cfg.user == defaultUser) {
      ${defaultUser} = {
        description = "pokebot server owner";
        group = defaultUser;
        home = cfg.webserver.dataDir;
        createHome = true;
        isSystemUser = true;
      };
    };

    users.groups = optionalAttrs (cfg.user == defaultUser) {
      ${defaultUser} = {
        members = [defaultUser];
      };
    };

    services.nginx = mkIf cfg.webserver.nginx.enable {
      enable = true;
      virtualHosts.${cfg.webserver.nginx.virtualHost.serverName} = lib.mkMerge [
        cfg.webserver.nginx.virtualHost
        {
          locations."~ /(api|swagger)/" = {
            proxyPass = "http://${cfg.webserver.bindAddress}";
            recommendedProxySettings = true;
          };
        }
      ];
    };
  };
}
