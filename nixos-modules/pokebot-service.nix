{
  config,
  pkgs,
  lib ? pkgs.lib,
  ...
}:
with lib; let
  cfg = config.services.onlyfan;
in {
  ###### interface
  options = {
    services.onlyfan = rec {
      enable = mkOption {
        type = types.bool;
        default = false;
        description = ''
          Whether to run pokebot
        '';
      };
    };
    # TODO: Add more options and generate config file
  };

  # https://discourse.nixos.org/t/how-to-create-generic-yaml-toml-ini-files/29797
  xdg.configFile."some/config.yaml".source = (pkgs.formats.toml { }).generate "something" {
    settings = {
      draw_bold_text_with_bright_colors = true;
      dynamic_title = true;
      live_config_reload = true;
      window.dimensions = {
        columns = 0;
        lines = 0;
      };
      scrolling = {
        history = 10000;
        multiplier = 3;
      };
  };
};

  ###### implementation

  config = mkIf cfg.enable {
    systemd.services.onlyfan = {
      wantedBy = ["multi-user.target"];
      after = ["network-online.target"];
      description = "TeamSpeak 3 Music Bot";
      serviceConfig = {
        ExecStart = "${pkgs.pokebot}/bin/pokebot";
        Restart = "always";
        RestartSec = 30;

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
        RestrictAddressFamilies = [];
        RestrictNamespaces = true;
        RestrictRealtime = true;
        NoNewPrivileges = true;
        PrivateDevices = true;
        PrivateTmp = true;

        # RuntimeDirectory = "onlyfan";
        # RootDirectory = "/run/onlyfan";
        #
        # BindReadOnlyPaths = [
        #   "/sys/class/hwmon/"
        # ];
        # ReadWritePaths= [
        #   "/sys/devices/platform/"
        # ];
      };
    };
  };
}
