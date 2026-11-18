{lib, extraFiles ? []}:
lib.fileset.toSource {
  root = ../.;
  fileset = lib.fileset.unions ([
    ../Cargo.toml
    ../Cargo.lock
    ../packages/splitcast-rs/Cargo.toml
    ../packages/splitcast-rs/src
    ../packages/splitcast-rs/tests
    ../packages/splitcast-rs/example.toml
    ../tools/xtask/Cargo.toml
    ../tools/xtask/src
  ] ++ extraFiles);
}
