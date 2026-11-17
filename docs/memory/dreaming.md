# Dreaming: knowledge consolidation and retrieval contract

Updated 2026-11-17. This page describes **how to maintain the knowledge map**; it is not a new product feature, implementation plan, test receipt, or permission to operate hosts.

## Basis and transfer boundary

Anthropic Engineering's [*The components of agent memory*](https://www.anthropic.com/research/components-of-agent-memory) (2026-11-06) distinguishes durable **memory notes** from navigable **memory representations** (such as a wiki). "Dreaming" can consolidate notes and session evidence into linked knowledge, correct mistakes, and promote a small subset into startup context. The article's evaluation does not prove that SplitCast's wiki has achieved those results. [*Effective context engineering for AI agents*](https://www.anthropic.com/research/effective-context-engineering-for-ai-agents) motivates progressive disclosure: fetch relevant pages rather than preloading the archive.

Apply that principle **without autonomous reinterpretation of evidence**. Markdown prose is a retrieval interface; source code and exact run receipts remain the authorities for what was implemented and observed.

## Memory layers and retrieval

| Layer | SplitCast location | Role | Update rule |
| --- | --- | --- | --- |
| Startup index | [`MEMORY.md`](../../MEMORY.md), [`INDEX.md`](INDEX.md) | Short mission, canonical entrypoints, and what not to repeat | Keep compact and current; no copied test transcripts |
| Knowledge map | [Mission](poc-mission.md), [decisions](decisions.md), [risks](open-risks.md), [working state](session-state.md) | Answer why, what changed, what is unproven, and where to look | Link relevant code, source IDs, competing claims, and correction provenance |
| Evidence register | [`evidence.md`](evidence.md), [verification](../verification.md), [dated receipts](../evidence/) | Bind claims to source revisions, environments, commands, expected/actual results | Append evidence; never relabel an older run as a new-head pass |
| Immutable historical material | `docs/history/`, old handoffs and Git objects | Original decisions and disproved hypotheses | Preserve and label as historical; do not silently make these operational instructions |
| Executable authority | `packages/`, `tools/`, `nix/`, `.github/workflows/` | Current behavior, configuration gates and CI contracts | Inspect exact revision; a wiki claim never overrides implementation |

At session start, read the startup index and **one domain page relevant to the task**, then retrieve exact code/evidence only for disputed claims. Do not automatically load the entire `docs/` tree or repeat the old handoff.

## Consolidation and contradiction protocol

1. **Collect:** Identify source file/commit, observed behavior or operator instruction, timestamp and environment. Classify each as `GOAL`, `SOURCE`, `OBSERVED`, `INFERRED`, or `UNKNOWN`. A test marked PASS must have its own actual artifact and contract.
2. **Connect:** Compare the current code with relevant decisions, source register, receipts and closed/merged PRs. Attach stable paths/commits; record where a later claim supersedes a dated earlier one. A PR merge does not imply deployment.
3. **Consolidate:** Merge duplicate prose into a *single current pointer*, but keep dated original evidence, source IDs and rejected options intact. Prefer a correction header over rewriting a historical observation.
4. **Contradict:** Seek a disconfirming example. In particular, verify media by decoded content rather than counters, stop by completed ownership-specific release rather than intent acknowledgement, and package delivery separately from runtime acceptance.
5. **Expose:** Update only affected current-state/index/risk references. Identify who/what is still blocked. Make claims falsifiable and narrow enough for the next experiment.
6. **Verify:** Check links and source identity; review the diff for credentials, host names and historical-result promotion. Run only checks needed for changed files, record any NOT RUN checks, and do not activate/merge/deploy.

## Current correction ledger (2026-11-17)

| Earlier/currently stale claim | Confirmed correction | Scope / still unknown |
| --- | --- | --- |
| PR #2 is unmerged; urgent Stop always waits behind Prepare | [PR #2](https://github.com/tenzyu/splitcast/pull/2) is merged (merge commit `6349fd6`). Listener-side urgent Stop admission is implemented. | Bounded **intent** acknowledgement is not guaranteed completed cleanup; parser-slot exhaustion, durable outcomes and cross-host replay remain |
| PR #3 is open/under review | [PR #3](https://github.com/tenzyu/splitcast/pull/3) is merged (merge commit `a5e7a22`). | Its production-only synthetic media tests do not establish capture-Agent/SSH, live frame-freshness enforcement, physical capture or gameplay non-regression |
| S20 original media timing claim was accepted without qualification | [AV01 / PR #3 follow-up](../evidence/pr3-review-20261117.md) corrected the oracle and added VFR/dropout negative controls. | Separate exact run receipts are required for newer revisions |
| Speech synthesis exists, therefore the required return audio is done | Current `agent.rs` rejects enabled TTS/EventSub/remote mixing and speech command delivery. | Independent production-to-capture playback is a **required user goal**, not accepted software |
| A `202 requested` Stop means failed release, or means completed release | [`server.rs`](../../packages/splitcast-rs/src/server.rs) may acknowledge an accepted Stop intent within a bounded interval; only a matching `completed` response proves completion. [CI regression](https://github.com/tenzyu/splitcast/actions/runs/44089362405) exposed an overly strict test expectation. | [PR #6](https://github.com/tenzyu/splitcast/pull/6) proposes a **test-only** correction; as of this record it is unmerged and final CI success has not been established |

The last row is a live work item, not an adopted implementation change. Recheck actual PR statuses and CI receipts before using these lines as current facts.

### Post-merge correction (later 2026-11-17)

The table captures an earlier snapshot. Subsequently [PR #5](https://github.com/tenzyu/splitcast/pull/5) merged at `9ef4431` and [PR #7](https://github.com/tenzyu/splitcast/pull/7) merged at `1678a0c`. PR #5's failed CI run is still a failed run, not retroactively passed by the merge. At this check [PR #6](https://github.com/tenzyu/splitcast/pull/6) remained open: `verify` and `verify-virtual` succeeded, but the package job was still running. This is a source-status update only; no capture, media acceptance, deployment, or completed CI is inferred.

## Next useful investigation

Start at the [environment-gap entry](environment-gap.md), **not** at the old cache-publication or generalized infrastructure checklist. Inventory the existing media and independent returned-audio capabilities at the capture/production boundary; distinguish gameplay interference, media freshness and returned-audio latency. Choose a bounded falsifiable software experiment before inventing another subsystem. The current experimental runtime stays disabled by default and host operations require independent approval.
