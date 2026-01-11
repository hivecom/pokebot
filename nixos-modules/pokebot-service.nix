{
  config,
  pkgs,
  lib ? pkgs.lib,
  ...
}:
with lib;
let
  cfg = config.services.pokebot;
in
{
  ###### interface
  options = {
    services.pokebot = {
      enable = mkOption {
        type = types.bool;
        default = false;
        description = ''
          Whether to run pokebot
        '';
      };
      package = lib.mkOption {
        type = lib.types.package;
        default = pkgs.pokebot;
        defaultText = "pkgs.pokebot";
        description = "Pokebot package";
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
          Location to look for music in
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
      wantedBy = [ "multi-user.target" ];
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];
      description = "TeamSpeak 3 Music Bot";
      serviceConfig = {
        ExecStart = "${lib.getExe cfg.package} /etc/pokebot/config.toml";
        Restart = "always";
        RestartSec = 30;
        WorkingDirectory = "/etc/pokebot";

        DynamicUser = true;
        StateDirectory = "pokebot";
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
        RestrictAddressFamilies = [ ];
        RestrictNamespaces = true;
        RestrictRealtime = true;
        NoNewPrivileges = true;
        PrivateDevices = true;
        PrivateTmp = true;
      };
    };
    # https://discourse.nixos.org/t/how-to-create-generic-yaml-toml-ini-files/29797
    # FIXME: security implications of being in the nix store?
    environment.etc."pokebot/config.toml".source = (pkgs.formats.toml { }).generate "pokebot-config" {
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

    services.nginx = mkIf cfg.webserver.nginx.enable {
      enable = true;
      virtualHosts = {
        ${cfg.webserver.domain} = {
          locations."/" = {
            proxyPass = "http://${cfg.webserver.bindAddress}";
            recommendedProxySettings = true;
          };
        };
      };
    };
  };
}
