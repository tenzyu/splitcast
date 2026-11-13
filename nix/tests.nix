{
  nixpkgs,
  homeManager,
  self,
}: let
  system = builtins.currentSystem;
  pkgs = import (nixpkgs.outPath or nixpkgs) {inherit system;};
  inherit (pkgs) lib;
  executable = pkgs.writeShellScriptBin "splitcast" ''
    printf '%s\0' "$@"
  '';
  package = executable.overrideAttrs (old: {
    passthru = (old.passthru or {}) // {
      example = ../packages/splitcast-rs/tests/example.json;
      rnnoise = pkgs.emptyDirectory;
      obs = pkgs.emptyDirectory;
    };
  });
  moduleSelf = self // {
    packages = (self.packages or {}) // {
      ${system}.default = package;
    };
  };
  hm = options: (import ((homeManager.outPath or homeManager) + "/modules") {
    inherit pkgs;
    configuration = {
      imports = [(import ./modules/home-manager.nix {self = moduleSelf;})];
      home.username = "splitcast-test";
      home.homeDirectory = "/var/empty/splitcast-test";
      home.stateVersion = "26.05";
      services.splitcast = options;
    };
  }).config;
  nixos = extra: (import ((nixpkgs.outPath or nixpkgs) + "/nixos/lib/eval-config.nix") {
    inherit system;
    modules = [./modules/nixos.nix {system.stateVersion = "26.05";} extra];
  }).config;
  off = hm {};
  gated = hm {enable = true; role = "production";};
  unusualPath = "/run/splitcast-test/test capture' $HOME %n.toml";
  candidate = hm {enable = true; rustCandidate = true; configFile = unusualPath;};
  gatedUnit = gated.systemd.user.services.splitcast-production;
  candidateUnit = candidate.systemd.user.services.splitcast-capture;
  summarize = cfg: {
    captureHelper = cfg.programs.gpu-screen-recorder.enable;
    pipewire = cfg.services.pipewire.enable;
    rtkit = cfg.security.rtkit.enable;
    graphics = cfg.hardware.graphics.enable;
    nftables = cfg.networking.nftables.enable;
    hasFirewall = cfg.networking.nftables.tables ? splitcast-obs-websocket-guard;
    hasDisplay = cfg.systemd.services ? splitcast-display;
  };
  systemOff = summarize (nixos {});
  production = summarize (nixos {services.splitcast = {enable = true; role = "production";};});
  capture = summarize (nixos {services.splitcast = {enable = true; role = "capture";};});
  firewallConfig = nixos {services.splitcast.stockObsFirewall = {enable = true; port = 4555;};};
  firewall = firewallConfig.networking.nftables.tables.splitcast-obs-websocket-guard;
  displayConfig = nixos {
    nixpkgs.config.allowUnfree = true;
    services.splitcast = {enable = true; role = "production"; display = {enable = true; user = "splitcast-test";};};
    users.users.splitcast-test.isNormalUser = true;
    services.xserver.videoDrivers = ["nvidia"];
    hardware.nvidia.open = false;
  };
  displayUnit = displayConfig.systemd.services.splitcast-display;
  failures = cfg: builtins.filter (a: !a.assertion && lib.hasPrefix "services.splitcast" a.message) cfg.assertions;
  invalidDisplay = nixos {services.splitcast.display.enable = true;};
  overrides = summarize (nixos {
    services.splitcast = {enable = true; role = "capture";};
    programs.gpu-screen-recorder.enable = false;
    services.pipewire.enable = false;
    hardware.graphics.enable = false;
    security.rtkit.enable = false;
  });
  rejected = options: !(builtins.tryEval (builtins.deepSeq (hm options).systemd.user.services true)).success;
  generate = role: settings: (hm {enable = true; inherit role settings;}).xdg.configFile."splitcast/${role}.toml".source;
  checks = {
    disabledUserUnit = !(lib.any (lib.hasPrefix "splitcast-") (builtins.attrNames off.systemd.user.services));
    migrationGate = lib.hasInfix "migration-gated" (builtins.head gatedUnit.Service.ExecStart);
    candidateCommand = lib.hasSuffix "splitcast-agent" (builtins.head candidateUnit.Service.ExecStart);
    cleanupCommand = lib.hasSuffix "splitcast-cleanup" gatedUnit.Service.ExecStopPost;
    manualUserUnit = gatedUnit.Install.WantedBy == [];
    cgroup = gatedUnit.Service.KillMode == "control-group";
    limits = gatedUnit.Service.CPUWeight == 25 && gatedUnit.Service.Nice == 10 && gatedUnit.Service.MemoryMax == "2G";
    privateUmask = gatedUnit.Service.UMask == "0077";
    singlePackage = builtins.length (builtins.filter (p: p == package) gated.home.packages) == 1;
    relativePathRejected = rejected {enable = true; configFile = "relative.toml";};
    roleDisagreementRejected = rejected {enable = true; role = "production"; settings.role = "capture";};
    invalidRoleRejected = rejected {enable = true; role = "invalid";};
    disabledSystem = !systemOff.captureHelper && !systemOff.nftables && !systemOff.hasDisplay;
    helperRoleGate = !production.captureHelper && capture.captureHelper;
    prerequisites = lib.all (cfg: cfg.pipewire && cfg.rtkit && cfg.graphics) [production capture];
    optionalFeaturesOff = lib.all (cfg: !cfg.nftables && !cfg.hasFirewall && !cfg.hasDisplay) [production capture];
    optionalFirewall = firewallConfig.networking.nftables.enable && firewall.family == "inet" && lib.hasInfix ''iifname != "lo" tcp dport 4555 counter drop'' firewall.content;
    manualDisplay = displayUnit.wantedBy == [];
    displayNoTcp = lib.hasInfix "-nolisten tcp" displayUnit.serviceConfig.ExecStart;
    privateDisplayCookie = displayUnit.serviceConfig.RuntimeDirectoryMode == "0711" && lib.hasInfix "chmod 0600" displayUnit.preStart;
    displayPrerequisites = failures displayConfig == [] && builtins.length (failures invalidDisplay) == 3;
    overridableDefaults = !overrides.captureHelper && !overrides.pipewire && !overrides.graphics && !overrides.rtkit;
  };
in {
  evaluation = {
    packageKind = "synthetic argument-printing binary, not the root flake package";
    inherit checks;
    passed = lib.all (value: value) (builtins.attrValues checks);
  };
  configs = pkgs.linkFarm "splitcast-module-config-tests" [
    {name = "capture.toml"; path = generate "capture" {};}
    {name = "production.toml"; path = generate "production" {};}
    {name = "overridden.toml"; path = generate "capture" {peer.ssh_host = "custom-host"; capture.width = 1920;};}
  ];
  commands = pkgs.linkFarm "splitcast-module-command-tests" [
    {name = "gated"; path = builtins.head gatedUnit.Service.ExecStart;}
    {name = "agent"; path = builtins.head candidateUnit.Service.ExecStart;}
    {name = "cleanup"; path = candidateUnit.Service.ExecStopPost;}
  ];
  expectedConfigPath = unusualPath;
}
