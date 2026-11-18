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
      splitcast-obs = splitcast.obs;
      splitcast-obs-stock-candidate = pkgs.obs-studio.override {browserSupport = false;};
      splitcast-voicevox = voicevoxPkgs.voicevox-engine;
    };
    devShells.${system} = {
      default = pkgs.mkShell {
        packages = with pkgs; [cargo rustc rustfmt clippy gcc pkg-config pulseaudio pipewire wireplumber python3 ffmpeg-full openssh coreutils cargo-audit cargo-deny];
      };
      media-test = pkgs.mkShell {
        inputsFrom = [self.devShells.${system}.default];
        packages = with pkgs; [mediamtx splitcast.obs rnnoise-plugin.ladspa xorg-server xauth mesa python3 bubblewrap git];
        SPLITCAST_RNNOISE_PLUGIN = "${pkgs.rnnoise-plugin.ladspa}/lib/ladspa/librnnoise_ladspa.so";
        LIBGL_DRIVERS_PATH = "${pkgs.mesa}/lib/dri";
        __EGL_VENDOR_LIBRARY_FILENAMES = "${pkgs.mesa}/share/glvnd/egl_vendor.d/50_mesa.json";
        SPLITCAST_XKB_ROOT = "${pkgs.xkeyboard-config}/share/X11/xkb";
      };
    };
    checks.${system} = {
      package = splitcast;
      configs = pkgs.runCommand "splitcast-config-check" {} ''
        ${splitcast}/bin/splitcast selfcheck
        ${splitcast}/bin/splitcast validate --config ${./packages/splitcast-rs/example.toml}
        ${splitcast}/bin/splitcast validate --config ${production}
        touch "$out"
      '';
    };
    nixosModules.default = ./nix/modules/nixos.nix;
    homeManagerModules.default = {
      key = "${self.outPath}/nix/modules/home-manager.nix";
      imports = [(import ./nix/modules/home-manager.nix {inherit self;})];
    };
  };
}
