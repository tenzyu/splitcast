# Current SplitCast working state

Recorded 2026-11-14 JST. This page is a cold-start capsule, not a new application test result.

## Goal and next task

The operator wants software development and virtual acceptance to proceed independently of physical performance acceptance. The next session implements GitHub Actions CI and a usable SplitCast Nix binary cache. Read [the handoff](../handoff/ci-nix-cache.md) before changing workflows or cache configuration. [Production-readiness gates](../production-readiness.md) define the work beyond CI.

## Source state

- Branch is `main`, with `origin/main` configured according to local Git status. Remote authentication, branch protection, and live remote contents were not checked for this handoff.
- HEAD is `aa86fe0d443d55a1a66c61198811a5b906ebef57`.
- The virtual-control implementation, new tests, and evidence documents are still uncommitted. A checkout or new worktree at HEAD alone does not contain them.
- New Rust files include `request.rs`, `server.rs`, `agent_lifecycle_tests.rs`, `tests/two_agents.rs`, and `tests/local_transport.rs` under `packages/splitcast-rs/`.
- No `.github/` workflow or project-declared binary-cache configuration was found in this handoff inspection. Do not infer the state of an external cache account or host-wide Nix settings.

Preserve the working tree. Review its tracked and untracked changes before committing or moving work. Never reset to HEAD to obtain a supposedly tested baseline.

## Last verified software artifact

[S16/E12](evidence.md) identifies the source as HEAD plus the virtual-control changes, not HEAD alone. The final [source manifest](../evidence/virtual-system-20261114-source-sha256.tsv) contains 49 source and configuration identities. The [command summary](../evidence/virtual-system-20261114-summary.tsv) records 14 passing steps, 124 ordinary tests, and three separately executed local components. These results belong to that run, not a new CI run or a new Nix package build.

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
