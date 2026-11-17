# Current SplitCast working state

Updated 2026-11-17. This page is a cold-start capsule. Distinct checks retain their own source and run identities below.

## Post-merge implementation checkpoint

S25's [convergence checkpoint](../evidence/poc-convergence-20261117.md) is current.
The fixture implementation and S24 Wiki are locally committed in `3c2f913`.
This continuation observed that commit already present and did not create or
publish it. It preserves the separate `ec38ee3` test change. Latest full verifier
passes eleven stages at clean `3c2f913`, with 165 tests per feature selection.
An earlier three-case reset failure remains recorded, not diagnosed as fixed.

Twitch connection, necessary Helix calls and existing suitable OAuth use are
approved. The selected channel and usable existing token are still absent in
the inspected deployment. No API call or new receiver was run. Read-only capture
inventory identifies absent GSR/helper, cached headless FFmpeg without Pulse
input, existing native audio tools and an untested portal alternative. Capture
SSH remains read-only. See the checkpoint for the narrow player-execution request.

PR #3 is merged at `a5e7a22`. S23's private readiness handoff and S24's new
operator instruction supersede the earlier pending-merge/next-discovery wording
below only for their stated scopes. Start at [the initial PoC checkpoint](../evidence/poc-20261117.md).
It records partial local playback work and independent GPU/engine evidence,
read-only capture inventory, finite recording limits, and exact remaining gates.
No real Twitch-triggered return, physical offload or controlled non-regression
result is claimed. The uncommitted state in the S24 record is historical and is
superseded by S25's local-commit observation above.

## Goal and next task

The current goal is [gameplay-first offload plus independent audio return](poc-mission.md)
(S21/D15), not expansion into a general distributed app. **The next environment-gap
session starts at [this entry](environment-gap.md).** No new PoC or host work is
authorized here. The Agent's TTS/EventSub refusal is a gap against that mission,
not evidence that the return path is no longer required.

PR #3 is open on `acceptance/real-media-plane`, originally published at `8486e81`
against `2a774d9`. The [2026-11-17 follow-up](../evidence/pr3-review-20261117.md)
tracks seven findings, current oracle fixes, bounded reset reproduction and
final-HEAD commands. Human review is separate and pending. No merge/deploy.

S22 implementation `fb632bf` passes all three gates: media run
`20261117T083058Z-ApaTam5y` (16 units, six cases, six recording controls and
recovery), ordinary run `1794904247771515533` (11 stages), virtual run
`1794904349298835682` (14 stages, 156 ordinary tests per feature selection and
three separate local opt-ins). Twenty focused reset attempts pass; cause remains
unknown. The first VFR negative fixture failed to generate, was corrected once,
then all six controls ran and rejected correctly. Final documentation-bearing
HEAD gets its own all-three-gate receipt in PR #3; do not promote these
implementation-head runs into a new-HEAD result.

## Preserved 2026-11-15 checkpoint (S20)

The following next-work recommendations are historical S20 proposals, not the
new session's assignment or an approved implementation sequence.

AV01 is corrected and tested at `42a64e4`: unique source-second audio, decoded PTS continuity, sampled transition uncertainty, boundary identity and stream endpoints replace the falsified `0accf63` phase-fit claim. Eleven pure tests, six real-media cases, and all three generated recording mutations pass. Partial EOF markers outside joint observation are explicitly unmeasured, not fitted away. Human review remains pending after the last reviewer returned no verdict due to provider quota.

The operator wants software acceptance independent of physical performance acceptance. S20/E16 verifies the production-only generated media path at `42a64e41990eb3a411dcd68e3412ccceac574112`. Real production Agent, MediaMTX, RNNoise/private Pulse, and patched OBS produce a locally confined MKV that an independent decoder inspects. Read [the exact receipt](../evidence/media-plane-20261115.md) for final regression outcomes and preserved failures, and [the profile](../media-acceptance.md), not an old stand-in test or cache result.

Next evidence-generating work is capture-Agent generated publication through a local SSH topology, actual media-network impairment without delaying the RTMP handshake, and an independently timestamped microphone marker at the intended VAD setting. Require both Agent statuses, freshly decoded output, known clock correspondence, and confirmed cleanup. The current four-second backlog is before RTMP ingress, not a network-delay result. Runtime liveness still does not enforce fresh content. P1 remains narrowed rather than closed.

S18's delivery receipt remains specific to `d2205d0`. It does not automatically deliver this candidate. No binary-cache publication rerun, deployment, hardware, privileged helper, or autostart is authorized. The finish directive authorizes one unmerged source PR to main, not merge or deployment. Current main at task start was `2a774d9`, which already includes PR #2 merge `6349fd6`. The older unmerged and Stop-behind-Prepare statements below are preserved historical checkpoints, not current main behavior.

## Source state

- S20's implementation commits are `0accf63`, `67720ca`, and `42a64e4`, based on clean `2a774d9`. The corrected media source manifest is `60987062daaf94d2a9fb57308edfe8756a4e54303f813eac53e2c22f2571e54b`. Later evidence/memory edits do not become new runtime executions. README, existing lockfiles, runtime gates, and other branches were not changed.

- `7585308` commits the prior virtual-control implementation and handoff. Its 49 source identities match S16's manifest. The initial S17 capsule at that commit describes the formerly dirty tree; it is historical, not current Git status.
- CI/cache implementation was merged through PR #1 into `main` at `869c965` after all four hosted jobs passed for head `65f8fb9`. The operator approved this ordinary merge and the separately gated verification prerelease, not deployment. The source-review checkpoint is `d37c387`; the later structural Nix-version correction has its own passing nine-test hosted result.
- Publisher correction `d2205d06b0dc695c3d9b7372fafa7facee769fa9` is the actual successful delivery source. Subsequent handoff/documentation commits do not become new runtime or hosted-package evidence merely by being newer.
- The existing public repository is `tenzyu/splitcast`, default `main`. Read-only inspection found Actions enabled and main unprotected. No repository visibility or branch-protection change was made.
- `NIX_CACHE_SIGNING_KEY` is configured as a GitHub repository secret. The matching public key is in `nix/cache-public-key`. Do not regenerate or rotate it implicitly. No private key belongs in Git, Nix derivations or evidence uploads.
- `.github/workflows/ci.yml` owns read-only hosted validation. `release-cache.yml` is manual, main-only, requires separate publication and redistribution confirmation, promotes only its own successful trusted build bundle, and tests consumption on a fresh runner.

Preserve the working tree. Review its tracked and untracked changes before committing or moving work. Never reset to HEAD to obtain a supposedly tested baseline.

## Last verified software artifact

S20's corrected `verify-media` run `20261115T022547Z-vrJRNZQD` passes eleven pure tests and six actual-media cases, including epoch 15 on the same controller after RNNoise death. The normal file is 11.900 seconds with 354 decoded frames; the maximum accepted sampled game A/V upper bound is 0.271 seconds including uncertainty. Frozen and backlogged content demonstrate progressing bytes without freshness. All final cleanup-error lists are empty; no cleanup retry was needed. Earlier `0accf63` regression passes retain their identities in [the receipt](../evidence/media-plane-20261115.md), but its timing claim is superseded by AV01.

Final-source `verify-virtual` run `1794709924594226389` failed no-default-features: initial capture Prepare reset its connection in `late_owned_start_cannot_recreate_a_completed_remote_session` before the replay assertion. The other thirteen stages passed. The single retry `1794710203786488803` passes all 14 stages at unchanged source: 155 ordinary tests per feature selection plus three separate local components. Final `verify` run `1794710355626646567` passes all 11 stages, including both feature selections. Cause of the reset remains unknown; no code was changed on speculation. All three required gates now have passing final-source runs; the earlier failure is not erased.

The initial unsandboxed OBS attempt opened a GPU render node and was rejected. Software mode alone is not device isolation. All accepted runs require the namespace launcher, which hides physical media devices, confines networking to its private loopback, and exposes only a fresh writable evidence directory. Never run the Python orchestrator directly or treat a selected-case exit as complete acceptance.

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
