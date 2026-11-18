{
  lib,
  rustPlatform,
  pkg-config,
  makeWrapper,
  fontconfig,
  libxcb,
  libxkbcommon,
  wayland,
  vulkan-loader,
}: let
  runtimeLibraries = [fontconfig libxcb libxkbcommon wayland vulkan-loader];
  subdir = "packages/splitcast-studio";
in
  rustPlatform.buildRustPackage {
    pname = "splitcast-studio";
    version = "0.1.0";
    src = import ../../nix/source.nix {
      inherit lib;
      extraFiles = [./Cargo.toml ./Cargo.lock ./src];
    };
    cargoRoot = subdir;
    buildAndTestSubdir = subdir;
    cargoLock.lockFile = ./Cargo.lock;
    nativeBuildInputs = [pkg-config makeWrapper];
    buildInputs = [libxcb libxkbcommon wayland];
    postInstall = ''
      wrapProgram "$out/bin/splitcast-studio" \
        --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath runtimeLibraries}
    '';
    passthru = {inherit runtimeLibraries;};
    meta = {
      mainProgram = "splitcast-studio";
      description = "Opt-in SplitCast Agent desktop control";
      license = lib.licenses.mit;
      platforms = lib.platforms.linux;
    };
  }
