{
  description = "SplitCast experimental Rust-owned capture and remote production";
  inputs.nixpkgs.url = "https://channels.nixos.org/nixos-26.05/nixexprs.tar.xz";

  outputs = {self, nixpkgs}: let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};
    splitcast = pkgs.callPackage ./packages/splitcast-rs {};
    voicevoxPkgs = import nixpkgs {
      inherit system;
      config.allowUnfreePredicate = package: pkgs.lib.getName package == "voicevox-onnxruntime";
    };
    toml = pkgs.formats.toml {};
    example = builtins.fromJSON (builtins.readFile splitcast.example);
    production = toml.generate "splitcast-production.toml" (pkgs.lib.recursiveUpdate example {
      role = "production";
      peer = {ssh_host = "capture-host"; control_port = 17871; remote_control_port = 17870; forward_control_port = 17873;};
    });
  in {
    packages.${system} = {
      default = splitcast;
      inherit splitcast;
      splitcast-rs-core = splitcast;
      splitcast-web = splitcast.web;
      splitcast-obs = splitcast.obs;
      splitcast-obs-stock-candidate = pkgs.obs-studio.override {browserSupport = false;};
      splitcast-voicevox = voicevoxPkgs.voicevox-engine;
    };
    devShells.${system}.default = pkgs.mkShell {
      packages = with pkgs; [cargo rustc rustfmt clippy gcc pkg-config pulseaudio ffmpeg-full openssh coreutils wasm-bindgen-cli lld cargo-audit cargo-deny];
    };
    checks.${system} = {
      package = splitcast;
      web = splitcast.web;
      configs = pkgs.runCommand "splitcast-config-check" {} ''
        ${splitcast}/bin/splitcast selfcheck
        ${splitcast}/bin/splitcast validate --config ${./packages/splitcast-rs/example.toml}
        ${splitcast}/bin/splitcast validate --config ${production}
        touch "$out"
      '';
    };
    nixosModules.default = import ./nix/modules/nixos.nix;
    homeManagerModules.default = import ./nix/modules/home-manager.nix {inherit self;};
  };
}
