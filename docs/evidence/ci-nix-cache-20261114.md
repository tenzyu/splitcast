# CI and cache implementation record

The operator requested committing the existing work first, then implementing
[the CI/cache handoff](../handoff/ci-nix-cache.md). Commit `7585308` preserves the
previous virtual-control implementation and evidence. All 49 source identities
matched its published manifest. `sha256sum --check docs/evidence/virtual-system-20261114-source-sha256.tsv`
exited 0 with one warning for the TSV header; every source entry was OK.
`git diff --check` exited 0 before that commit.

## Live boundaries

Read-only GitHub inspection found the existing public repository
`tenzyu/splitcast`, default branch `main`, and authenticated administrator access.
Remote main was `aa86fe0d443d55a1a66c61198811a5b906ebef57`. Actions are enabled.
`gh api repos/tenzyu/splitcast/branches/main/protection` returned HTTP 404,
`Branch not protected`, exit 1. No branch protection was changed.
Repository Actions secrets and variables were both empty. No cache account,
approved destination, verified signing key, or write credential has been supplied.
Cache publication and clean-cache consumption are therefore not yet accepted.

## Local commands

Commands run from the standalone repository; failures are retained here rather
than superseded by later success. No physical capture or deployment is authorized.

| Exact command | Result |
| --- | --- |
| `nix develop -c rustc --print target-libdir --target wasm32-unknown-unknown` | Exit 0; Nix compiler reports its WASM library directory. Existence is checked by preparation below. |
| `bash tools/ci/identity.sh` | Exit 0; source, lock hashes, Nix version, platform and disk observations recorded under ignored `artifacts/ci/`. |
| `nix develop -c bash tools/ci/prepare.sh` | Exit 0; Rust 1.95.0 matches the declared toolchain, actual WASM libstd exists, Cargo 1.95.0, wasm-bindgen 0.2.121; locked registry fetch succeeds. This local registry was already warm. |
| `nix shell --inputs-from . nixpkgs#actionlint nixpkgs#shellcheck -c bash -euo pipefail -c 'actionlint; shellcheck tools/ci/*.sh'` | Initial exit 1; actionlint passed, ShellCheck SC2046 rejected unquoted output-list expansion. Changed to a Bash array rather than suppressing the check. Rerun pending. |
| `nix shell --inputs-from . nixpkgs#actionlint nixpkgs#shellcheck -c bash -euo pipefail -c 'actionlint; shellcheck tools/ci/*.sh'` | After the Bash-array fix, exit 0; both linters passed. |

## Design and safety

Validation runs ordinary `verify`, extended `verify-virtual`, and packaging on
separate ephemeral GitHub-hosted Ubuntu 24.04 runners. PRs receive only read-only
repository access. Checkout credentials are not persisted. Third-party action
commits were resolved from official release tags using GitHub's refs API:

| Action | Release | Immutable commit |
| --- | --- | --- |
| actions/checkout | v7.0.1 | `3d3c42e5aac5ba805825da76410c181273ba90b1` |
| actions/upload-artifact | v7.0.2 | `cf430e030ddbb5b0abf93d22962f4752f3646cd9` |
| cachix/install-nix-action | v31.11.5 | `3dc140bfc76c49fbd4dd958429132e3746bcb086` |

No acceleration cache or PR-generated artifact is restored. Evidence uploads
allowlist verifier log/identity/summary files and packaging metadata; fixture
state, tokens and unrelated `artifacts/` contents are not uploaded. Workflow job
deadlines are finite. No Home Manager input was added: `nix/tests.nix`'s module
assertions remain outside `nix flake check`, explicitly not claimed by CI.

Runtime gates and autostart settings remain unchanged. Rollback consists of
reverting only the CI changes in a new commit; preserve `7585308`, historical
evidence and failed runs. There is no host activation to undo.
