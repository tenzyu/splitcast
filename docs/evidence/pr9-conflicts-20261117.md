# PR #9 base-conflict resolution

The operator requested conflict resolution after opening PR #9. This authorizes
incorporating the base into the feature branch, not merging the PR into main,
deployment or physical operations.

## Source and resolution choices

- Feature parent: `e93d068ebdd134ab3ac7dee65b786deef0290023`.
- Incorporated base: `c76e930`, including README/Dreaming updates, the PR #6
  test-only asynchronous Stop acknowledgement correction at `e20065d`, and CI
  cache automation. The inherited statements that PR #6 was open remain dated
  snapshots, not current source availability.
- `git merge --no-commit --no-ff origin/main` exits 1 with two conflicted files:
  `MEMORY.md` and `docs/memory/open-risks.md`.
- `MEMORY.md` keeps the later PoC checkpoint, granted Twitch operations, missing
  inputs and read-only capture SSH boundary, plus main's Dreaming protocol link
  and dated source/CI evidence.
- `docs/memory/open-risks.md` keeps both the PoC findings and Dreaming correction
  sections. No failure, unknown cause or physical acceptance gate is removed.
- Automatically merged index, environment-gap and session-state changes retain
  the newer PoC entrypoints and main's historical-source corrections. The index
  and handoff now distinguish checkpoint/PR/conflict-resolution authorization
  from a PR merge or deployment grant.
- Runtime source and locks remain unchanged from the feature parent. The only
  inherited Rust change is main's existing test correction. README, CI scripts,
  the test correction and Dreaming protocol are preserved from the base.

## Verification

The resolved combined tree passes the ordinary verifier. Commands run directly
in the reviewed Nix/direnv environment retain the normal repository wrapper
equivalence. The merge was not yet committed when the verifier ran, so the run
identifies `e93d068` plus the resolved merge contents, not a clean feature-only
tree. Subsequent edits record documentation results only.

| Exact command | Actual result |
| --- | --- |
| `cargo run -p splitcast-xtask -- verify` | Exit 0, all eleven stages. Run `artifacts/verification/runs/1794922559908362495/` has 165 passing tests in each default and no-default-feature selection. Formatting, check, Clippy and feature checks pass. |
| `python3 artifacts/poc/20261117-pr9-conflicts/check-resolution.py` | Exit 0. No unmerged index entries or conflict markers. Feature runtime, locks and runtime gates match the feature parent. Base README, Dreaming protocol, CI files and Stop test match the base exactly. Both risk records, links and publication hygiene checks pass. |
| `bash tools/ci/lint.sh` | Exit 127 before lint runs because `actionlint` is absent on PATH. |
| `nix shell --offline --no-write-lock-file --inputs-from . nixpkgs#actionlint nixpkgs#shellcheck nixpkgs#python3Packages.pyflakes -c bash tools/ci/lint.sh` | Exit 1 provisioning tools. A fixed-output dependency has a hash mismatch. No source hash, lock or package definition was changed to bypass it. Full CI-script lint remains unverified locally. |
| `python3 tools/ci/test_finalize_release.py` | Exit 0, three offline mocked tests. Printed release URLs are fixture output, not real publication. |
| `SPLITCAST_CACHE_PROTOCOL_TEST=1 python3 tools/ci/test_release_cache.py` | Exit 1. Nine unit tests pass, but the opt-in Nix protocol fixture errors when its path-based source import encounters a retained special file in ignored historical media artifacts. Those artifacts were not deleted or modified. |
| `python3 tools/ci/test_release_cache.py` | Exit 0, nine unit tests pass, with the Nix protocol test explicitly skipped. This is not a protocol pass or full lint pass. |

Private command logs and source identities stay Git-ignored under
`artifacts/poc/20261117-pr9-conflicts/`. No private data is added to the merge.
Hosted checks, virtual/media acceptance, full Nix package builds and physical
tests were not run as part of this resolution. Local conflict resolution and
ordinary verification do not establish all PR checks green.

No physical capture, media recording, Twitch call, service activation or deployment
is part of conflict resolution. Earlier 165-test and native-fixture receipts keep
their original source identity; they are not a pass for the combined tree.
