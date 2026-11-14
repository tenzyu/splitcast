# Current SplitCast working state

Updated 2026-11-14 JST. This page is a cold-start capsule. Distinct checks retain their own source and run identities below.

## Goal and next task

The operator wants software development and virtual acceptance to proceed independently of physical performance acceptance. S18 implements [the CI/cache handoff](../handoff/ci-nix-cache.md) using signed GitHub Releases, not a Cachix account. Read [its actual results](../evidence/ci-nix-cache-20261114.md) before rerunning publication. [Production-readiness gates](../production-readiness.md) define the work beyond delivery.

## Source state

- `7585308` commits the prior virtual-control implementation and handoff. Its 49 source identities match S16's manifest. The initial S17 capsule at that commit describes the formerly dirty tree; it is historical, not current Git status.
- CI/cache implementation was merged through PR #1 into `main` at `869c965` after all four hosted jobs passed for head `65f8fb9`. The operator approved this ordinary merge and the separately gated verification prerelease, not deployment. The source-review checkpoint is `d37c387`; the later structural Nix-version correction has its own passing nine-test hosted result.
- The existing public repository is `tenzyu/splitcast`, default `main`. Read-only inspection found Actions enabled and main unprotected. No repository visibility or branch-protection change was made.
- `NIX_CACHE_SIGNING_KEY` is configured as a GitHub repository secret. The matching public key is in `nix/cache-public-key`. Do not regenerate or rotate it implicitly. No private key belongs in Git, Nix derivations or evidence uploads.
- `.github/workflows/ci.yml` owns read-only hosted validation. `release-cache.yml` is manual, main-only, requires separate publication and redistribution confirmation, promotes only its own successful trusted build bundle, and tests consumption on a fresh runner.

Preserve the working tree. Review its tracked and untracked changes before committing or moving work. Never reset to HEAD to obtain a supposedly tested baseline.

## Last verified software artifact

[S16/E12](evidence.md) identifies its original dirty source, now preserved by `7585308`. Its final [source manifest](../evidence/virtual-system-20261114-source-sha256.tsv) contains 49 identities. Its [command summary](../evidence/virtual-system-20261114-summary.tsv) records 14 passing steps, 124 ordinary tests, and three separate local components. [S18/E14](evidence.md) adds actual clean hosted CI, new package builds and cache-specific evidence without relabeling S16's results. Full local cache preparation passes at `d37c387`; only S18's explicit hosted consumer result can establish public substitution.

The failed hosted `d37c387` run revealed an extra empty `build-trace-v2`
directory under Nix 2.35.6. Do not restore fixed-name cleanup. Flat cache
preparation now removes empty directories structurally and refuses nonempty
ones. The correction passes hosted run `43558332265`. Release run `43559084940`
then passed its trusted build and signing but failed before publication because
the public tag endpoint does not resolve drafts. Draft `445034829` with 452
matching signed assets remains unpublished and preserved. Do not retry the old
code or publish that draft implicitly. The ID-from-create correction requires a
new trusted run and its explicit consumer result before delivery is complete.

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
