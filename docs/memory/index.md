---
okf_version: "0.2"
---
# SplitCast knowledge bundle — progressive retrieval

Load [MEMORY.md](../../MEMORY.md) once for the mission and permission boundary; then **only the relevant topic** below. The code and source-bound receipts outrank summaries.

## Goal and decisions
* [PoC mission](poc-mission.md) — workload isolation, separate media and return-audio budgets; why not a general streaming platform
* [Decisions D01–D20](decisions.md) — retained choices, rejected alternatives, correction dates and source IDs
* [Open risks](open-risks.md) — falsifiable unresolved gates, failure modes, and authorization limits

## Implementation and acceptance
* [Which revision implements it?](proposed-revisions.md) — main, #18, the #19/#20 stack, CI and local integration lineage
* [Prepare 409](proposed-revisions.md#prepare-returns-409) · [Studio loses focus](proposed-revisions.md#chat-or-preview-stalls-after-focus-transfer) · [Advancing bytes, stale content](proposed-revisions.md#bytes-advance-while-the-scene-ages) — choose one diagnosis and follow its pinned test
* [What proves Stop?](open-risks.md#stop-admission-and-release) — matching ticket, owner release, OBS exit and remaining resources
* [Physical state](session-state.md) — merged #16 receipts, speech-only versus on-air, outstanding host gates
* [Completed environment discovery](environment-gap.md) — **deprecated** historical investigation, not a fresh task
* [PoC handoff](../handoff/poc-20261117.md) — dated November 17 attempts and then-missing Twitch inputs
* [Operations](../operations.md) — stop/rollback and runtime containment; not deployment approval

## Evidence and memory maintenance
* [Evidence IDs](evidence.md) — Sxx/Exx identity and original artifact boundaries
* [Real VOICEVOX checkpoints](../evidence/real-voicevox-physical-return.md) — physical return, fault recovery, and the tested utterance expiry
* [Verification record](../verification.md) — separate CI, virtual, generated media and physical acceptance
* [Dreaming protocol](dreaming.md) — re-check note versus source, merge contradictions and prune redundant text
* [Update protocol](update-protocol.md) — annotate corrections without overwriting failed results
* [Change log](log.md) — dated wiki maintenance

## Forensic/immutable originals
* [Archived complete pre-Dreaming Markdown](https://github.com/tenzyu/splitcast/tree/archive/pre-dreaming-20261117) — source of pruned verbose reports and full review transcripts, retrieved only if a specific claim requires it
* [Historical ADR](../history/source/docs/graphics-streaming/ADR-2026-11-07-performance-first.md) — original performance-first decision retained in current checkout
* [Historical decision collection](../history/source/docs/ARCHITECTURE_DECISIONS.md) — historically scoped, not current operating permissions

For a new solution, check the [engineering policy](../../RUST_ENGINEERING_AGENTS.md), [semantic review](../design-review.md), [Agent protocol](../../packages/splitcast-rs/src/protocol.rs) and [existing virtual fixtures](../virtual-acceptance.md). Search the pinned source with `git grep -n PATTERN REV -- path`.

No automatic preload of historical sources or evidence matrices. Cross-links reflect **retrieval routes**, not agreement with a superseded claim.
