#!/usr/bin/env bash
set -euo pipefail

rustc --version --verbose
cargo --version
wasm-bindgen --version
expected=$(sed -n 's/^channel = "\([^"]*\)"$/\1/p' rust-toolchain.toml)
actual=$(rustc --version)
[[ -n "$expected" && "$actual" == "rustc $expected "* ]]
shopt -s nullglob
stdlib=("$(rustc --print target-libdir --target wasm32-unknown-unknown)"/libstd-*.rlib)
(( ${#stdlib[@]} > 0 ))
cargo fetch --locked
