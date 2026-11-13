{consumer}: let
  inherit (consumer.inputs.nixpkgs) lib;
  candidates = lib.concatMap (configuration:
    map (home: {system = configuration.config; inherit home;})
      (builtins.attrValues (configuration.config.home-manager.users or {})))
    (builtins.attrValues consumer.nixosConfigurations);
  selected = builtins.filter (candidate:
    (candidate.home.services.splitcast.settings.peer.ssh_host or "") != "") candidates;
  summarize = candidate: let
    cfg = candidate.home.services.splitcast;
    unit = candidate.home.systemd.user.services."splitcast-${cfg.role}" or null;
  in {
    inherit (cfg) enable role rustCandidate;
    packagePath = cfg.package.outPath;
    systemEnabled = candidate.system.services.splitcast.enable;
    captureHelper = candidate.system.programs.gpu-screen-recorder.enable;
    servicePresent = unit != null;
    wantedBy = if unit == null then [] else unit.Install.WantedBy;
    migrationGated = unit == null || lib.any (lib.hasInfix "migration-gated") unit.Service.ExecStart;
    cgroup = unit == null || unit.Service.KillMode == "control-group";
    packageFromInput = cfg.package.outPath == consumer.inputs.splitcast.packages.${candidate.system.nixpkgs.hostPlatform.system}.default.outPath;
  };
  roles = builtins.listToAttrs (map (candidate: {
    name = candidate.home.services.splitcast.role;
    value = summarize candidate;
  }) selected);
  checks = {
    twoRoles = builtins.length selected == 2 && roles ? capture && roles ? production;
    captureDisabled = !roles.capture.enable && !roles.capture.systemEnabled && !roles.capture.servicePresent && !roles.capture.captureHelper;
    productionIntegrated = roles.production.enable && roles.production.systemEnabled && roles.production.servicePresent;
    runtimeGatesClosed = lib.all (cfg: !cfg.rustCandidate && cfg.migrationGated && cfg.wantedBy == [] && cfg.cgroup) (builtins.attrValues roles);
    externalPackages = lib.all (cfg: cfg.packageFromInput) (builtins.attrValues roles);
  };
in {
  inputRevision = consumer.inputs.splitcast.rev or "uncommitted-or-path-input";
  inherit roles checks;
  passed = lib.all (value: value) (builtins.attrValues checks);
}
