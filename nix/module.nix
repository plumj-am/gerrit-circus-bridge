{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.gerrit-circus-bridge;
  mkStringOption =
    default:
    lib.mkOption {
      inherit default;
      type = lib.types.str;
    };
in
{
  options.services.gerrit-circus-bridge = {
    enable = lib.mkEnableOption "Gerrit Circus CI Bridge";

    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.gerrit-circus-bridge;
      defaultText = lib.literalExpression "pkgs.gerrit-circus-bridge";
      description = "The gerrit-circus-bridge package.";
    };

    circusUrl = mkStringOption "https://circus.plumj.am";
    gerritQuery = mkStringOption "status:open+-is:wip";

    circusApiKeyFile = lib.mkOption {
      type = lib.types.path;
      description = "File containing Circus API key (needs eval-jobset role).";
    };

    gerritUrl = mkStringOption "https://gerrit.plumj.am";
    gerritUsername = lib.mkOption {
      type = lib.types.str;
      default = "circus";
      description = "Gerrit HTTP user for REST API.";
    };
    gerritPasswordFile = lib.mkOption {
      type = lib.types.path;
      description = "File containing Gerrit HTTP password.";
    };

    pollInterval = lib.mkOption {
      type = lib.types.int;
      default = 30;
      description = "Seconds between Gerrit poll cycles.";
    };

    pollTimeout = lib.mkOption {
      type = lib.types.int;
      default = 3600;
      description = "Max seconds to wait for builds.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.gerrit-circus-bridge = {
      description = "Gerrit Circus CI Bridge";
      wantedBy = [ "multi-user.target" ];
      wants = [ "network-online.target" ];
      after = [ "network-online.target" ];
      restartIfChanged = true;

      serviceConfig = {
        Type = "exec";
        User = "circus";
        Restart = "on-failure";
        RestartSec = "5s";
        StateDirectory = "gerrit-circus-bridge";
        StateDirectoryMode = "0750";

        Environment = [
          "CIRCUS_URL=${cfg.circusUrl}"
          "GERRIT_URL=${cfg.gerritUrl}"
          "GERRIT_USERNAME=${cfg.gerritUsername}"
          "GERRIT_CHANGE_QUERY=${cfg.gerritQuery}"
          "POLL_INTERVAL=${toString cfg.pollInterval}"
          "POLL_TIMEOUT=${toString cfg.pollTimeout}"
        ];

        EnvironmentFile = [
          cfg.circusApiKeyFile
          cfg.gerritPasswordFile
        ];

        ExecStart = "${cfg.package}/bin/gerrit-circus-bridge";

        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        RestrictSUIDSGID = true;
      };
    };
  };
}
