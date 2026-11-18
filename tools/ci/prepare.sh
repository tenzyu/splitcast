#!/usr/bin/env bash
set -euo pipefail

rustc --version --verbose
cargo --version
expected=$(sed -n 's/^channel = "\([^"]*\)"$/\1/p' rust-toolchain.toml)
actual=$(rustc --version)
[[ -n "$expected" && "$actual" == "rustc $expected "* ]]
cargo fetch --locked
