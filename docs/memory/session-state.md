# Current SplitCast working state

Updated 2026-11-14 JST. This page is a cold-start capsule. Distinct checks retain their own source and run identities below.

## Goal and next task

The operator wants software development and virtual acceptance to proceed independently of physical performance acceptance. S18 completes [the CI/cache handoff](../handoff/ci-nix-cache.md) for the named `d2205d0` release using signed GitHub Releases, not a Cachix account. Next is P1: generated media through the actual agent RTMP and RTSP path. Do not substitute the separate TCP component or stand-in worker tests for that requirement. Read [the delivery results](../evidence/ci-nix-cache-20261114.md) before rerunning publication, and [the readiness gates](../production-readiness.md) before expanding scope.

## Source state

- `7585308` commits the prior virtual-control implementation and handoff. Its 49 source identities match S16's manifest. The initial S17 capsule at that commit describes the formerly dirty tree; it is historical, not current Git status.
- CI/cache implementation was merged through PR #1 into `main` at `869c965` after all four hosted jobs passed for head `65f8fb9`. The operator approved this ordinary merge and the separately gated verification prerelease, not deployment. The source-review checkpoint is `d37c387`; the later structural Nix-version correction has its own passing nine-test hosted result.
- Publisher correction `d2205d06b0dc695c3d9b7372fafa7facee769fa9` is the actual successful delivery source. Subsequent handoff/documentation commits do not become new runtime or hosted-package evidence merely by being newer.
- The existing public repository is `tenzyu/splitcast`, default `main`. Read-only inspection found Actions enabled and main unprotected. No repository visibility or branch-protection change was made.
- `NIX_CACHE_SIGNING_KEY` is configured as a GitHub repository secret. The matching public key is in `nix/cache-public-key`. Do not regenerate or rotate it implicitly. No private key belongs in Git, Nix derivations or evidence uploads.
- `.github/workflows/ci.yml` owns read-only hosted validation. `release-cache.yml` is manual, main-only, requires separate publication and redistribution confirmation, promotes only its own successful trusted build bundle, and tests consumption on a fresh runner.

Preserve the working tree. Review its tracked and untracked changes before committing or moving work. Never reset to HEAD to obtain a supposedly tested baseline.

## Last verified software artifact

[S16/E12](evidence.md) identifies its original dirty source, now preserved by `7585308`. Its final [source manifest](../evidence/virtual-system-20261114-source-sha256.tsv) contains 49 identities. Its [command summary](../evidence/virtual-system-20261114-summary.tsv) records 14 passing steps, 124 ordinary tests, and three separate local components. [S18/E14](evidence.md) adds actual clean hosted CI and package delivery without relabeling S16's results. Run `43561723796` passes build, publish and fresh no-build consume at `d2205d0`. See [the durable receipt](../evidence/ci-nix-cache-20261114-receipt.json) and [actual download commands](../binary-cache.md).

The failed hosted `d37c387` run revealed an extra empty `build-trace-v2`
directory under Nix 2.35.6. Do not restore fixed-name cleanup. Flat cache
preparation now removes empty directories structurally and refuses nonempty
ones. The correction passes hosted run `43558332265`. Release run `43559084940`
then passed its trusted build and signing but failed before publication because
the public tag endpoint does not resolve drafts. Draft `445034829` with 452
matching signed assets remains unpublished and preserved. Do not retry the old
code or publish that draft implicitly. The ID-from-create correction succeeds
in run `43561723796`; public prerelease `445047931` has the verified 452 assets.
Its tag is `nix-cache-d2205d06b0dc695c3d9b7372fafa7facee769fa9-43561723796-1`.
The official 439 NARs remain upstream, while only three custom NARs are hosted
on GitHub. Different consumer inputs need their own proof.

The two-agent tests use actual capture and production agent processes but stand-in external workers. Generated TCP transfer and full decode are a separate test. Actual agent RTMP and RTSP media integration is not accepted.

## Corrections that must not be lost

- V01 fixed unrecognized HTTP status responses losing the unknown-outcome distinction.
- V02 fixed opt-in test selection returning zero tests while the verifier reported PASS.
- V03 corrected three review-only settings-file assertions. `Agent::new` appends the role directory. Observe `agent.cfg.state_dir/settings.json`, not the original fixture directory.
- V03's original instrument is preserved with its original hash. The corrected instrument has a positive file-write control and fails on an actual forbidden write under mutation. Its later lifecycle suite and verifier passed. The evidence establishes sampled persistent-state observations, not exhaustive scheduling or absence of every possible transient effect.

See [the complete run and correction record](../evidence/virtual-system-20261114.md). Do not restore the invalid review claims when summarizing a PASS verdict.

## Open work and safety

Ordinary Stop still queues behind Prepare. Lease maintenance remains serial. Actual media-path integration, congestion behavior, settings durability, browser request ownership, typed validation/error boundaries, and declared crash-recovery semantics need further work. See [open risks](open-risks.md) and the readiness gates rather than repeating the original extraction review as if no repairs occurred.

Direct Scanout, GPU contention, capture permissions, real device provenance, physical audio, real OBS acceptance, and deployment containment remain separately authorized acceptance work. Keep `rustCandidate=false` and autostart targets empty.

`artifacts/` contains ignored local evidence, including original and corrected review instruments. It is not a publication payload. The portable source manifest and summary are under `docs/evidence/`. The canonical decision trail is `.audit/virtual-system.tsv`. Do not require the temporary review worktrees to run ordinary verification.

## 2026-11-14 control-plane branch follow-up

PR #2 remains a separate, unmerged control-plane candidate. Its
`review/urgent-control` implementation `5125799` supersedes the historical
Stop-behind-Prepare statement above. [S19/E15](evidence.md) records the fixes,
red/green checks, and final-head verification location. Resume from the actual
PR head, not an older private branch. Keep raw private history unpublished.

Matching expired owners can finalize held release, and remote Prepare ownership
starts at a positive socket write. Ephemeral completion, late Prepare replay,
and parser-slot exhaustion remain explicit limits. This work does not take over
README or actual-media-path tasks and does not authorize merge or deployment.
