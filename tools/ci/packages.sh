#!/usr/bin/env bash
set -euo pipefail

mkdir -p artifacts/ci
nix derivation show .#default > artifacts/ci/derivations.json
nix build .#default --no-link --print-out-paths | tee artifacts/ci/outputs.txt
mapfile -t outputs < artifacts/ci/outputs.txt
nix path-info --recursive --json --json-format 1 "${outputs[@]}" > artifacts/ci/closure.json
nix flake check
native=$(nix path-info .#default)
"$native/bin/splitcast" selfcheck
"$native/bin/splitcast" validate --config packages/splitcast-rs/example.toml
df -h > artifacts/ci/disk-after.txt
