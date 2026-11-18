---
type: Memory Maintenance Protocol
title: Dreaming and evidence corrections
description: Replay claims against code and receipts; prune only with a recoverable source identity.
status: stable
sources:
  - { id: anthropic, resource: https://www.anthropic.com/research/components-of-agent-memory, title: The components of agent memory }
  - { id: okf, resource: https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md, title: OKF v0.2 }
---
# Dreaming contract

Anthropic's 2026-11-06 memory work distinguishes **transcripts**, **memory notes**, **wiki-style representations**, and **small injected context**. Dreaming is a reconciliation pass, **not** prose shortening alone: replay uncertain notes against raw source, correct contradiction, link dispersed facts, prune redundant/stale summaries and promote only durable/high-use information into startup memory. Its experiments do **not** prove that this particular SplitCast corpus preserves retrieval performance. An oversized wiki that simply mirrors original documents loses the abstraction advantage.

## SplitCast allocation

| Memory layer | Source of truth / artifact | Consumption |
| --- | --- | --- |
| Source/transcripts | Exact Git ref, code, CI/job, evidence receipt, [archive](https://github.com/tenzyu/splitcast/tree/archive/pre-dreaming-20261117) | Pull on demand, never preload |
| Episodic/source register | [Evidence IDs](evidence.md), dated `docs/evidence/`, [log](log.md) | Claims scoped to environment/commit/time; retain failures |
| Semantic OKF v0.2 | [index](index.md), [decisions](decisions.md), [mission](poc-mission.md), [risks](open-risks.md), [state](session-state.md) | Cross-linked concepts with `type`, lifecycle and `sources` metadata |
| Injected startup | [MEMORY.md](../../MEMORY.md) and [AGENTS.md](../../AGENTS.md) | Short purpose/authority/next input; only task-relevant links |

**Do not claim `verified: human:` for model-written content; `status: stable` is a lifecycle hint, not proof of runtime acceptance.** A code inspection is `SOURCE`, an executed test with receipt is `OBSERVED`, a proposed performance change is `INFERRED`, and missing inputs are `UNKNOWN`.

## A bounded Dreaming pass

1. **Identify exact revision** (repo/ref, source tree, runtime if independently known), question and one domain page; stop before automatically loading unrelated Markdown.
2. **Replay/contradict:** retrieve relevant source code and exact dated receipt. Ask what falsifies the summary. Examples: advancing RTMP bytes but frozen frames; `202 requested` vs confirmed Stop cleanup; 30-fps header vs decoded VFR; fixture audio vs real event/physical hearing.
3. **Consolidate:** retain decision ID, earlier claim, rationale, rejected option, counterexample, corrected result and remaining unknown. Preserve exact negative-run identity; do not rewrite it as PASS after retries.
4. **Prune:** delete only duplicated or superseded active-tree text whose original is still addressable via a stable Git archive ref. Keep original ADRs and normative safety/license/runbooks. Repair inbound links, source IDs and task entrypoint.
5. **Promote:** update only [state](session-state.md), [risks](open-risks.md), [evidence](evidence.md), [index](index.md) and [startup](../../MEMORY.md) affected by the observation; include a dated [log](log.md) record.
6. **Verify:** compare bytes and token proxies before/after, parse OKF YAML and links, check source IDs/ADR integrity, re-run *relevant* application gates if code changed. Every unrun check stays `NOT RUN`.

## Retained corrections that must not be lost

- PR #2 urgent Stop admission merged; no hard release guarantee; PR #6 fixed an overly strict test expectation, not universal cancellation.
- PR #3 actual **production-only generated** media evidence is narrower than capture-Agent/SSH/physical acceptance. AV01 phase-fit timing was falsified; later decoded-PTS/audio-window controls corrected it.
- PR #9 finite `poc-play` merged; it is local fixture-only, Agent continues refusing TTS/EventSub/remote mixing.
- Twitch WSS/necessary Helix/`channel.chat.message` v1 with **existing suitable** OAuth approved; selected account/channel data missing in inspected scope; new OAuth/chat post/public output unauthorized.
- Game-side renderer hook caused an observed EGL path crash, **not** a proven Mesa root cause; Direct Scanout and local game audio must be protected.
- All-frame gameplay OFF/ON/OFF, live freshness and physical event-to-playback are separate acceptance axes.

For correction fields and evidence discipline see [update protocol](update-protocol.md). For exact pre-pruning prose and full original correction tables use [archive](https://github.com/tenzyu/splitcast/tree/archive/pre-dreaming-20261117).

## Incremental proposals

[Provenance](provenance.json) identifies the exact local paths to compare for
each page. Its `watch_from` is a change-detection anchor. Manual review may add
`review` with a full `commit` that contains the reviewed page and sources, plus
`evidence_commit` and `evidence_path` for the durable review record. The scan
reports the evidence blob and checks that the reviewed page and dependencies
still match. It never certifies the meaning of a claim. Record any resulting
correction as specified by the [update protocol](update-protocol.md).

Use [the CLI workflow](../convergence.md) to scan locally or explicitly ask
Codex for a private, source-bound proposal. Decisions about applying a patch
and recording its review stay with the reviewer.
