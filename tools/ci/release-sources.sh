#!/usr/bin/env bash
set -euo pipefail

directory=${1:?usage: bash tools/ci/release-sources.sh OUTPUT_DIRECTORY}
mkdir -p "$directory"
git archive HEAD Cargo.toml Cargo.lock rust-toolchain.toml flake.nix flake.lock \
  packages nix tools/xtask LICENSE LICENSES THIRD_PARTY.md | xz -T2 > "$directory/project-source.tar.xz"
vendor=$(nix eval --raw .#default.cargoDeps)
obs=$(nix eval --raw .#splitcast-obs.src)
nixpkgs=$(nix flake archive --json | python3 -c 'import sys,json; print(json.load(sys.stdin)["inputs"]["nixpkgs"]["path"])')
tar -cJh --sort=name --mtime=@0 --owner=0 --group=0 -C "$vendor" . > "$directory/cargo-sources.tar.xz"
tar -cJ --sort=name --mtime=@0 --owner=0 --group=0 -C "$obs" . > "$directory/obs-source.tar.xz"
tar -cJ --sort=name --mtime=@0 --owner=0 --group=0 -C "$nixpkgs" . > "$directory/nixpkgs-source.tar.xz"
cp THIRD_PARTY.md "$directory/THIRD_PARTY.md"
