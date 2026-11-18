{self}: {
  config,
  lib,
  pkgs,
  ...
}: let
  cfg = config.services.splitcast;
  json = pkgs.formats.json {};
  toml = pkgs.formats.toml {};
  serviceName = "splitcast-${cfg.role}";
  gated = pkgs.writeShellScript "splitcast-migration-gated" ''
    echo 'SplitCast service is disabled pending Rust parity and explicit cutover approval.' >&2
    exit 1
  '';
  command = action: pkgs.writeShellScript "splitcast-${action}" ''
    exec ${lib.getExe cfg.package} ${action} --config ${lib.escapeShellArg cfg.configFile}
  '';
  defaults = builtins.fromJSON (builtins.readFile cfg.package.example);
  roleDefaults = {
    role = cfg.role;
    audio.rnnoise_plugin = "${cfg.package.rnnoise}/lib/ladspa/librnnoise_ladspa.so";
    obs.binary = "${cfg.package.obs}/bin/obs";
    peer = {
      ssh_host = if cfg.role == "production" then "capture-host" else "production-host";
    } // lib.optionalAttrs (cfg.role == "production") {
      control_port = 17871;
      remote_control_port = 17870;
      forward_control_port = 17873;
      token_file = "~/.config/splitcast/production.token";
      remote_token_file = "~/.config/splitcast/control.token";
    };
  };
  settings = lib.recursiveUpdate (lib.recursiveUpdate defaults roleDefaults) cfg.settings;
  generated = toml.generate "${serviceName}.toml" settings;
in {
  options.services.splitcast = {
    enable = lib.mkEnableOption "the manually started SplitCast user agent";
    role = lib.mkOption {
      type = lib.types.enum ["capture" "production"];
      default = "capture";
      description = "Agent role. Only one role can be configured per user.";
    };
    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      defaultText = lib.literalExpression "splitcast.packages.\${pkgs.stdenv.hostPlatform.system}.default";
      description = "SplitCast package with example, rnnoise, and obs passthru attributes.";
    };
    rustCandidate = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Explicit experimental Rust agent selection. False keeps ExecStart migration-gated.";
    };
    settings = lib.mkOption {
      type = lib.types.attrsOf json.type;
      default = {};
      description = "Overrides merged into package.example and role defaults to generate TOML. Use credential paths, never secret values.";
    };
    configFile = lib.mkOption {
      type = lib.types.str;
      default = toString generated;
      defaultText = lib.literalExpression "generated TOML from package.example and settings";
      description = "Absolute configuration path passed to the agent and cleanup. A runtime path allows mutable configuration outside the store.";
    };
    environment = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      default = {};
      example = {
        DISPLAY = ":99";
        XAUTHORITY = "/run/splitcast-display/Xauthority";
      };
      description = "Agent environment, including any separately managed display. Do not put secrets here.";
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = lib.hasPrefix "/" cfg.configFile;
        message = "services.splitcast.configFile must be an absolute path.";
      }
      {
        assertion = settings.role == cfg.role;
        message = "services.splitcast.settings.role must match services.splitcast.role.";
      }
    ];
    home.packages = [cfg.package];
    xdg.configFile."splitcast/${cfg.role}.toml".source = generated;
    systemd.user.services.${serviceName} = {
      Unit = {
        Description = "SplitCast ${cfg.role} control agent";
        After = ["network.target"];
        StartLimitIntervalSec = 60;
        StartLimitBurst = 3;
      };
      Service = {
        Type = "simple";
        LimitCORE = 0;
        ExecStart = if cfg.rustCandidate then "${command "agent"}" else "${gated}";
        Environment = lib.mapAttrsToList (name: value: "${name}=${value}") cfg.environment;
        Restart = "on-failure";
        RestartSec = 5;
        KillMode = "control-group";
        TimeoutStopSec = 20;
        ExecStopPost = "${command "cleanup"}";
        SendSIGKILL = true;
        UMask = "0077";
        WorkingDirectory = config.home.homeDirectory;
        CPUWeight = 25;
        Nice = 10;
        MemoryMax = "2G";
      };
      Install.WantedBy = [];
    };
  };
}
