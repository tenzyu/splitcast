{
  config,
  lib,
  ...
}: let
  cfg = config.services.splitcast.stockObsFirewall;
in {
  options.services.splitcast.stockObsFirewall = {
    enable = lib.mkEnableOption "the nftables guard for a stock OBS WebSocket listener (changes the firewall backend)";
    port = lib.mkOption {
      type = lib.types.port;
      default = 4455;
      description = "OBS WebSocket TCP port blocked on every non-loopback interface, for IPv4 and IPv6.";
    };
  };

  config = lib.mkIf cfg.enable {
    networking.nftables = {
      enable = true;
      tables."splitcast-obs-websocket-guard" = {
        family = "inet";
        content = ''
          chain input {
            type filter hook input priority -150; policy accept;
            iifname != "lo" tcp dport ${toString cfg.port} counter drop
          }
        '';
      };
    };
  };
}
