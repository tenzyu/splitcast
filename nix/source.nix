{lib}:
lib.fileset.toSource {
  root = ../.;
  fileset = lib.fileset.unions [
    ../Cargo.toml
    ../Cargo.lock
    ../packages/splitcast-rs/Cargo.toml
    ../packages/splitcast-rs/src
    ../packages/splitcast-rs/tests
    ../packages/splitcast-rs/example.toml
    ../packages/splitcast-protocol/Cargo.toml
    ../packages/splitcast-protocol/src
    ../packages/splitcast-web/Cargo.toml
    ../packages/splitcast-web/src
    ../packages/splitcast-web/assets
    ../tools/xtask/Cargo.toml
    ../tools/xtask/src
  ];
}
