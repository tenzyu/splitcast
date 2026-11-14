#!/usr/bin/env bash
set -euo pipefail

mkdir -p artifacts/ci
git rev-parse HEAD > artifacts/ci/revision.txt
git status --short > artifacts/ci/working-tree.txt
sha256sum Cargo.lock flake.lock > artifacts/ci/locks.txt
uname -a > artifacts/ci/platform.txt
nix --version > artifacts/ci/nix.txt
df -h > artifacts/ci/disk-before.txt
