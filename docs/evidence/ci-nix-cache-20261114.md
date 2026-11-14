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

## Source review, permission, and export correction

The actual initial export identified exactly three custom paths: the native
and web outputs above and
`/nix/store/azxbfi1l0rp0wz329qqvyiah1xi71js8-obs-studio-splitcast-32.1.2`.
All 439 other closure members matched official-cache metadata. No additional
custom dependency was allowed. The generated source archives were inspected:
project build/lock/patch/GPL inputs exist, the OBS WebSocket license and source
exist, the vendored Cargo archive contains crate sources and 151 license/notice
entries, and the Nixpkgs archive contains OBS build expressions and its upstream
patch. These counts describe this inspected archive, not a universal license
completeness metric. Archive-presence checks are now executable in preparation.

Terra's read-only implementation review found no other demonstrated security or
correctness blocker, but correctly blocked publication without this actual
source review and owner confirmation. After the above inspection, the operator
was told that the selected OBS includes GPL-2.0-or-later and FDK-AAC, that Nixpkgs'
redistributability declaration is not a legal/patent audit, and that only these
three NARs would be hosted with corresponding source. The operator explicitly
answered **「この構成で公開を承認」**. Separately, the operator authorized merging
PR #1 after verification and running the public fresh-consumer trial, with no
service activation. The workflow now requires a distinct
`reviewed_redistribution` confirmation as well as `publish`, both false by default.

| Exact command | Result |
| --- | --- |
| `bash -o pipefail -c 'bash tools/ci/release-sources.sh artifacts/release-sources-local 2>&1 \| tee artifacts/ci/local-release-sources.log'` | Exit 0 at `cf26f0d`; sources produced. Concurrent evaluation emitted ignored SQLite-busy diagnostics. |
| `bash -o pipefail -c 'nix develop -c cargo run -p splitcast-xtask -- verify 2>&1 \| tee artifacts/ci/local-verify-release.log'` | Exit 0 at clean `cf26f0d`; all 11 steps passed in `1794629927576783101`. |
| `bash -o pipefail -c 'python3 tools/ci/release_cache.py prepare artifacts/release-bundle-local artifacts/release-sources-local 2>&1 \| tee artifacts/ci/local-cache-prepare.log'` | Exit 1. It identified all 442 entries and exported the three custom NARs, then refused Nix's empty `log` directory as an unsafe asset. Nothing was published. |
| `env TMPDIR=/tmp/opencode SPLITCAST_CACHE_PROTOCOL_TEST=1 python3 tools/ci/test_release_cache.py NixProtocolTests` | First regression exit 1 with `unsafe release asset: log`. A more precise assertion rerun exited 1 with `['log', 'realisations'] != []`. After initial cleanup it passed, but a new post-sign check reproduced `['log', 'nar', 'realisations'] != []`, exit 1. Nix recreates these directories on opening the file store, not only during export. |
| `env TMPDIR=/tmp/opencode nix shell --inputs-from . nixpkgs#actionlint nixpkgs#shellcheck nixpkgs#python3Packages.pyflakes -c bash tools/ci/lint.sh` | Exit 0 after centralizing post-operation cleanup and source checks. All eight tests passed, including the real signing/empty-store fixture, bad signature/NAR controls, nonempty-directory preservation, and refusal of fake source archives. |
| `python3 -c 'from tools.ci.release_cache import verify_sources; from pathlib import Path; verify_sources(Path("artifacts/release-sources-local"))'` | Exit 0 on actual generated archives. |

The correction removes only known empty `nar`, `log`, and `realisations`
directories after file-cache operations. Nonempty directories still fail and
are preserved. It does not loosen the flat asset inventory or erase the failed
initial export directory. The source checks and explicit owner confirmation
address Terra's source-review prerequisite; they are not presented as a legal
opinion.

## Corrected full local export

| Exact command | Result |
| --- | --- |
| `bash -o pipefail -c 'bash tools/ci/release-sources.sh artifacts/release-sources-d37c387 2>&1 \| tee artifacts/ci/local-sources-d37c387.log'` | Exit 0 on clean `d37c387`. |
| `bash -o pipefail -c 'python3 tools/ci/release_cache.py prepare artifacts/release-bundle-d37c387 artifacts/release-sources-d37c387 2>&1 \| tee artifacts/ci/local-cache-d37c387.log'` | Exit 0; source contents checked, all 439 forwarded entries passed Nix signature verification, and all three custom NARs passed content verification. 452 flat assets including manifest. Manifest SHA-256 `d1346e212b0e127255844f441edfdba592e66ebc668426cbaaa67a173d7bf407`. Custom signatures are added later by the isolated publisher, not claimed here. |
| `bash -o pipefail -c 'nix develop -c cargo run -p splitcast-xtask -- verify-virtual 2>&1 \| tee artifacts/ci/local-virtual-d37c387.log'` | Exit 0; 14 PASS rows in `1794630585676086902`, on clean `d37c387`. |
| `bash -o pipefail -c 'nix develop -c cargo run -p splitcast-xtask -- verify 2>&1 \| tee artifacts/ci/local-ordinary-d37c387.log'` | Exit 0; 11 PASS rows in `1794630808754911017`, on clean `d37c387`. |
| `git diff --exit-code 7585308 HEAD -- packages Cargo.toml Cargo.lock flake.nix flake.lock nix/modules nix/obs.nix nix/source.nix` | Exit 0; application source, product packages, locks, flake, runtime modules and selected OBS configuration are unchanged by the delivery implementation. |

The exact archived-source evaluation below exited 0. It establishes identical
derivation output identities from the distributed project source, not another
full compilation or a consumer substitution pass:

```sh
python3 - <<'PY'
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
manifest = json.loads(Path('artifacts/release-bundle-d37c387/manifest.json').read_text())
with tempfile.TemporaryDirectory(prefix='splitcast-source-proof-', dir='/tmp/opencode') as directory:
    with tarfile.open('artifacts/release-sources-d37c387/project-source.tar.xz') as archive:
        archive.extractall(directory, filter='data')
    actual = {name: subprocess.check_output(['nix', 'eval', '--raw', f'path:{directory}#{attribute}.outPath'], text=True).strip() for name, attribute in [('native', 'default'), ('web', 'splitcast-web')]}
    assert actual == manifest['roots'], (actual, manifest['roots'])
    print(json.dumps({'archived_source_evaluates_to_exact_roots': actual}, indent=2))
PY
```

Terra's follow-up accepts that the earlier source/redistribution prerequisite is
addressed for the explicitly approved three-output configuration. It reports no
new concrete blocker in its limited review. It did not independently recover the
operator transcript or run the full exporter/hosted consumer. Its follow-up is
retained in `/tmp/opencode/splitcast-ci-review/terra.md`; the actual full exporter
and verifier results above are primary-session observations.

## Hosted Nix-version correction

[Run 43556826868](https://github.com/tenzyu/splitcast/actions/runs/43556826868)
on PR head `d37c387db7db60d7cd8ab93ef48d088174881c24` failed only
`Delivery scripts`. Ordinary, virtual and package jobs succeeded. The failed
protocol assertion was `['build-trace-v2'] != []`. Its retained `nix.txt` reports
Nix 2.35.6, while the local tool is 2.34.9. The premise that file-cache bookkeeping
directory names were stable across versions was false. The fix therefore does
not append another name to the list. A flat export removes empty directory
entries regardless of name and still refuses/preserves every nonempty directory
and symlink. No files or directory contents are recursively deleted.

| Exact command | Result |
| --- | --- |
| `gh run watch 43556826868 --interval 60 --exit-status > artifacts/ci/watch-43556826868.log 2>&1` | Exit 1. The hosted failure is retained rather than called an infrastructure-only pass. |
| `gh run view 43556826868 --log-failed` | Exit 0; revealed the actual assertion above. |
| `gh run download 43556826868 --name delivery-scripts-43556826868-1 --dir artifacts/ci/hosted-failed-delivery-43556826868` | Exit 0; exact failing script log and Nix identity retained locally. |
| `python3 tools/ci/test_release_cache.py MetadataTests.test_empty_bookkeeping_directory_names_are_not_a_nix_version_contract` | Before the structural fix, exit 1 with `['build-trace-v2', 'future-empty-directory'] != []`. |
| `env TMPDIR=/tmp/opencode nix shell --inputs-from . nixpkgs#actionlint nixpkgs#shellcheck nixpkgs#python3Packages.pyflakes -c bash tools/ci/lint.sh` | After the fix, exit 0; all nine tests, actionlint, ShellCheck and Pyflakes pass. |

This supersedes the earlier fixed-name cleanup design, not its failure history.
The pending source/memory documentation was preserved throughout the correction.

## Hosted correction accepted and ordinary merge

[Run 43558332265](https://github.com/tenzyu/splitcast/actions/runs/43558332265)
passed all four jobs for PR head
`65f8fb9caff5740200dbc4183a09dfb9ecab75cb`. Actual checkout identity is
`058a099fedcfee87a0414147e33fea6f719a6722`, GitHub's synthetic merge commit.
Downloaded ordinary run `1794631663970197646` has 11 PASS rows; extended run
`1794631663551048932` has 14 PASS rows. `Delivery scripts` actually runs and
passes nine tests under Nix 2.35.6. Packaged native/web outputs remain identical
to the earlier paths above.

| Exact command | Result |
| --- | --- |
| `bash -o pipefail -c 'nix develop -c cargo run -p splitcast-xtask -- verify 2>&1 \| tee artifacts/ci/local-ordinary-65f8fb9.log'` | Exit 0 on clean `65f8fb9`; all 11 steps pass in `1794631735865345025`. |
| `gh run watch 43558332265 --interval 60 --exit-status > artifacts/ci/watch-43558332265.log 2>&1` | Exit 0; actual hosted job conclusions subsequently inspected. |
| `gh run download 43558332265 --dir artifacts/ci/hosted-43558332265` | Exit 0; actual revision, summaries, package outputs and nine-test result inspected. |
| `gh pr ready 1` | Exit 0; removed draft status without merging. |
| `gh pr merge 1 --merge --match-head-commit 65f8fb9caff5740200dbc4183a09dfb9ecab75cb` | Exit 0 after all four jobs succeeded and with operator approval. PR API confirms MERGED at `2026-11-14T04:55:56Z`, merge commit `869c965ef2b9751f8e3aee44ad221478a6669fb1`. No squash, force push, tag rewrite or branch deletion. |
| `git fetch origin main` then `git switch main` then `git merge --ff-only origin/main` | Each exit 0; primary checkout is clean main at the confirmed merge. |
| `gh workflow run release-cache.yml --ref main -f publish=true -f reviewed_redistribution=true` | Exit 0; starts [release run 43559084940](https://github.com/tenzyu/splitcast/actions/runs/43559084940) for `869c965`. This dispatch is not yet a publication or consumer pass. |

The release workflow repeats ordinary/virtual/package validation on its own
trusted unprivileged runner. It does not promote the preceding PR artifacts.
The above PR checks are retained as separate evidence rather than substituted
for that required trusted build.

## First trusted release attempt: signing succeeds, draft lookup fails

[Release run 43559084940](https://github.com/tenzyu/splitcast/actions/runs/43559084940)
on `869c965` finishes **failure**. Its trusted `build` succeeds, including both
verifiers, packages, source checks and full unsigned export. The isolated
publisher's signing step succeeds and verifies the signed index. Upload also
succeeds for all 452 assets. The subsequent public tag-lookup endpoint returns
HTTP 404 for the still-draft release, so publication stops and `consume` is
skipped. This is a publication failure, not a failed software build or a cache
consumption pass.

Read-only inspection finds draft ID `445034829`, tag
`nix-cache-869c965ef2b9751f8e3aee44ad221478a6669fb1-43559084940-1`, still
`draft=true`. It is preserved unchanged. Its actual uploaded names, sizes and
SHA-256 digests all match the signed manifest, whose SHA-256 is
`a28b123d7af8f649de2d8c56849db0b8a66de1e9f821f4d4a21723c088a7ac0d`.
Do not publish this old draft implicitly or rerun the obsolete tag-lookup code.

| Exact command | Result |
| --- | --- |
| `gh run watch 43559084940 --interval 60 --exit-status > artifacts/ci/watch-43559084940.log 2>&1` | Exit 1. Build succeeds, publish fails, consume skipped. |
| `gh run view 43559084940 --log-failed > artifacts/ci/release-43559084940-failed.log` | Exit 0; original publish HTTP 404 retained. |
| `gh run download 43559084940 --name release-publisher-evidence-1 --dir artifacts/ci/release-43559084940-publisher` | Exit 0; signed manifest and tag evidence retained. |
| `gh run download 43559084940 --name release-build-evidence-1 --dir artifacts/ci/release-43559084940-build` | Exit 0; trusted build evidence retained. |
| `gh api 'repos/tenzyu/splitcast/releases/tags/nix-cache-869c965ef2b9751f8e3aee44ad221478a6669fb1-43559084940-1' --jq .id` | Exit 1, HTTP 404. Directly reproduces the inappropriate draft lookup. |
| `gh api repos/tenzyu/splitcast/releases/445034829 --jq '{id,tag_name,draft,target_commitish,assets:(.assets\|length)}'` | Exit 0, draft exists with 452 assets. This is the distinguishing positive control for lookup by ID. |
| `gh api --paginate --slurp 'repos/tenzyu/splitcast/releases/445034829/assets?per_page=100' > artifacts/ci/draft-445034829-assets.json` | Exit 0; all asset metadata obtained without making the draft public. |
| `python3 -c 'import json; from pathlib import Path; from tools.ci.release_cache import verify_uploaded_assets; print(verify_uploaded_assets(Path("artifacts/ci/release-43559084940-publisher/release-bundle"), json.loads(Path("artifacts/ci/draft-445034829-assets.json").read_text())))'` | Exit 0, prints `452`; every uploaded digest/size/name matches the actual signed manifest. |
| `env TMPDIR=/tmp/opencode nix shell --inputs-from . nixpkgs#actionlint nixpkgs#shellcheck nixpkgs#python3Packages.pyflakes -c bash tools/ci/lint.sh` | After the correction, exit 0 with ten passing tests. New asset-verification controls reject missing assets, duplicates, wrong digest and wrong size. |

The publisher now captures the release ID from the create response and uses that
same ID to enumerate and publish the draft. It does not depend on the public
tag endpoint before publication. Creation explicitly names the exact source SHA.
The asset comparison was extracted without changing its contract, so the actual
draft bytes and the workflow use the same rerunnable verifier. A new trusted
run is still required to prove the corrected publication path and cold consumer.

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
