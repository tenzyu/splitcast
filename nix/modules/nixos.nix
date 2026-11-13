{
  config,
  lib,
  pkgs,
  ...
}: let
  cfg = config.services.splitcast;
in {
  imports = [
    ./headless-display.nix
    ./stock-obs-firewall.nix
  ];

  options.services.splitcast = {
    enable = lib.mkEnableOption "SplitCast system prerequisites";
    role = lib.mkOption {
      type = lib.types.enum ["capture" "production"];
      default = "capture";
      description = "Role whose system prerequisites are enabled. The user agent is configured separately through Home Manager.";
    };
  };

  config = lib.mkIf cfg.enable {
    programs.gpu-screen-recorder.enable = lib.mkIf (cfg.role == "capture") (lib.mkDefault true);
    security.rtkit.enable = lib.mkDefault true;
    services.pipewire = {
      enable = lib.mkDefault true;
      pulse.enable = lib.mkDefault true;
      alsa.enable = lib.mkDefault true;
      wireplumber.enable = lib.mkDefault true;
    };
    hardware.graphics.enable = lib.mkDefault true;
    environment.systemPackages = [pkgs.v4l-utils];
  };
}
