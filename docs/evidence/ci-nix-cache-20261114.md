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
| `bash -o pipefail -c 'nix develop -c cargo run -p splitcast-xtask -- verify 2>&1 \| tee artifacts/ci/local-verify-initial.log'` | Exit 0; all 11 steps pass in `artifacts/verification/runs/1794628386372872591/`. The command began before committing the CI files and is not a clean-checkout claim. |
| `bash -o pipefail -c 'bash tools/ci/packages.sh 2>&1 \| tee artifacts/ci/local-packages-initial.log'` | Exit 0; native and WASM packages, flake checks, packaged selfcheck and example validate pass. Closure has 442 paths and 2,238,074,632 uncompressed NAR bytes. Initial evaluation emitted an ignored SQLite-busy diagnostic; Nix also warned about implicit JSON format and the Home Manager output name. |
| `gh run watch 43553937275 --exit-status` | Exit 0; all three hosted jobs succeeded. |
| `gh run download 43553937275 --dir artifacts/ci/hosted-43553937275` | Exit 0; inspectable source, lock, verifier and package evidence downloaded. |

## Initial hosted result

[Run 43553937275](https://github.com/tenzyu/splitcast/actions/runs/43553937275)
tested PR head `8d2d625ccf8e2c64718cfb2d54f7599bb01b4942` through GitHub's
synthetic merge checkout. The checkout's actual revision is retained in each
artifact. Jobs `verify`, `verify-virtual`, and `Native and WASM packages` all
succeeded on fresh ephemeral runners. This is CI evidence, not cache consumption
or deployment acceptance.

Downloaded artifacts identify the actual merge checkout as
`955b503f7c9ce2e8785c45afcc185ff2fc7e6aac` in all three jobs. Ordinary run
`1794628481410868515` has 11 PASS rows. Virtual run `1794628480380642523` has
14 PASS rows. Hosted native and web outputs exactly match the initial local
package outputs:

- Native: `/nix/store/jr2y8giph253yyq89rgy1w3l0dzc11fd-splitcast-rs-core-0.1.0`
- Web: `/nix/store/y7h4pdailgaaiia2nj913l2bi8j719jd-splitcast-web-0.1.0`

The earlier `/bin/sh` fixture concern did not fail either package build. No
source or test was weakened in response to that unreproduced concern.

## Approved change in cache transport

The operator proposed GitHub Releases using `hiraginoyuki/onair`'s workflow and
README as examples, then explicitly approved a signed Releases cache, signing-key
generation and repository-secret storage, and a verification prerelease after
redistribution checks. This supersedes the unavailable external-cache account
boundary above. It does not authorize deployment, forced tag updates, or history
rewrites. The existing source-built flake remains authoritative.

`nix shell --inputs-from . nixpkgs#shellcheck -c shellcheck tools/ci/create-cache-key.sh`
exited 0. With the operator's approval,
`env TMPDIR=/tmp/opencode bash tools/ci/create-cache-key.sh tenzyu/splitcast`
exited 0. The secret list confirms `NIX_CACHE_SIGNING_KEY`, updated at
`2026-11-14T04:07:33Z`. The only printed key was the public key, retained in
`nix/cache-public-key`. The temporary private key was removed by the script's
exit trap. This is not a publication or substitution result.

## Release implementation checks

| Exact command | Result |
| --- | --- |
| `nix path-info --json --json-format 2 .#default` | Exit 0; format 2 uses an `info` envelope and relative paths. The exporter instead explicitly selects format 1, which matches the existing full-path closure records. |
| `nix eval --json .#splitcast-obs.meta.license` | Exit 0; upstream metadata declares GPL-2.0-or-later and FDK-AAC, both redistributable. These declarations are not a legal or patent audit. |
| `nix eval --json .#splitcast-obs.patches` | Exit 0; both upstream Nix plugin-path patch and project loopback patch are retained. Corresponding sources and build expressions are selected for release assets. |
| `env TMPDIR=/tmp/opencode SPLITCAST_CACHE_PROTOCOL_TEST=1 python3 tools/ci/test_release_cache.py` | Exit 0; four tests passed. A generated disposable key signs a real flat file cache. A separate empty local store receives the exact fixture bytes. Unsigned metadata and modified signed NAR size were rejected as untrusted; modified compressed NAR bytes failed with `input compression not recognized`. These are expected negative controls, not product failures. No sandbox setting was disabled. |
| `nix shell --inputs-from . nixpkgs#shellcheck nixpkgs#python3Packages.pyflakes -c bash -euo pipefail -c 'shellcheck tools/ci/*.sh; pyflakes tools/ci/*.py'` | Exit 0 at the initial exporter checkpoint. |
| `nix shell --inputs-from . nixpkgs#actionlint nixpkgs#shellcheck nixpkgs#python3Packages.pyflakes -c bash -euo pipefail -c 'actionlint; shellcheck tools/ci/*.sh; pyflakes tools/ci/*.py'` | Exit 0 after adding the release workflow and publisher. |

The release design compares three isolated candidates and retains a Terra
cross-judgment. The chosen design promotes only a successful same-run trusted
unprivileged build bundle. The signing job does not build product derivations or
run bundle content. See [the design](../ci-release-design.md). Local candidate
sketches and the judgment are retained under
`/tmp/opencode/splitcast-ci-design/`; the synthesis is the committed durable
record. Release and fresh hosted consumption remain pending at this checkpoint.

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
