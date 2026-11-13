{lib,rustPlatform,stdenv,wasm-bindgen-cli,lld}: rustPlatform.buildRustPackage {
  pname = "splitcast-web";
  version = "0.1.0";
  src = import ../../nix/source.nix {inherit lib;};
  sourceRoot = "source";
  cargoLock.lockFile = ../../Cargo.lock;
  nativeBuildInputs = [wasm-bindgen-cli lld];
  buildPhase = ''
    runHook preBuild
    cargo build -p splitcast-web --locked --offline --release --target wasm32-unknown-unknown
    runHook postBuild
  '';
  checkPhase = ''
    cargo test -p splitcast-web -p splitcast-protocol --locked --offline --target ${stdenv.hostPlatform.rust.rustcTarget}
  '';
  installPhase = ''
    mkdir -p "$out"
    cp packages/splitcast-web/assets/index.html packages/splitcast-web/assets/loader.js "$out/"
    wasm-bindgen --target web --out-dir "$out" --out-name splitcast_web target/wasm32-unknown-unknown/release/splitcast_web.wasm
  '';
  meta.description = "Rust WASM client for the authenticated SplitCast loopback agent";
}
