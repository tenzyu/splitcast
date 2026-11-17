# Next session: environment-gap entry

Updated 2026-11-17. **This is the entry for the completed S23 environment-gap
session.** Continue from [S25's convergence checkpoint](../evidence/poc-convergence-20261117.md).
The following discovery instructions retain their earlier scope, not a new
prohibition against the operator's separately authorized S24 trials.
Read [the mission](poc-mission.md), then pull only the sources needed
for the question. Do not start from the old CI/cache or media-expansion task list.

## First establish the gap, without starting a new PoC here

1. Recheck current branch/PR state and [the merged PR #3 receipt](../evidence/pr3-review-20261117.md).
   A successful PR gate is not merge, deployment or physical acceptance.
2. Inventory capabilities needed at each [responsibility boundary](poc-mission.md#responsibility-and-dependency-boundaries):
   selected acquisition and audio tap, unavoidable local encoding, production
   receive/processing, independent returned-audio playback, and failure isolation.
   Compare **existing software capabilities** with requirements before proposing
   a new control system or transport. No host access is authorized by this page.
3. Keep user goal / source fact / confirmed observation / reviewer hypothesis /
   unverified candidate distinct. Attach source IDs, commits, date and scope.
   Code facts are in `agent.rs::prepare`, `media.rs::game/mic/tunnel/rnnoise`,
   `obs.rs` and `tts.rs`; they are not evidence of running host capabilities.
4. Identify which missing fact would change the next decision. Obtain separate
   authorization before private host access, hardware capture, microphone use,
   privilege installation, listener exposure or runtime activation. Store any
   authorized machine inventory and access mappings privately using role labels
   in published conclusions. No machine-specific inventory is collected here.
5. Propose a small discriminating experiment only after defining its budget and
   observable failure behavior. Gameplay interference, media freshness and audio
   return are independent budgets. Neither a TTS-first implementation order nor
   a transport/latency target has been approved.

## Known before another investigation

| Known | Do not repeat / assume |
| --- | --- |
| Production-only generated media traverses actual Agent/MediaMTX/RNNoise/private Pulse/OBS (E16). | Not capture-Agent/SSH, physical acquisition, competitive performance or live freshness enforcement. |
| Current Agent refuses TTS/EventSub/remote speech mixing (S21/D15). | Do not mark the required return path delivered, or remove it from the mission because the code refuses it. |
| Source backlog is before RTMP; frozen/backlogged bytes still advance. | Do not rename it network congestion or infer freshness from counters. |
| AV01 and PR #3 follow-up expose acceptance-oracle holes. | Preserve superseded results; inspect current oracle contracts and negative controls rather than reusing old PASS claims. |
| P0 delivery closed only for `d2205d0`; PRs #2 and #3 are merged into main. | Do not repeat cache publication, label urgent Stop as still universally queued, or infer current deployment. |

## Handoff stop conditions

No full architecture rewrite, new PoC, GUI, actual speech-engine connection,
capture, host switch or publication was performed by this review/Wiki session.
The next session must choose scope explicitly. P1 is still narrowed, not closed;
the bidirectional goal and gameplay non-interference remain unaccepted.
Read [risks](open-risks.md) and [the update protocol](update-protocol.md) before
changing a knowledge claim. [PR #3](https://github.com/tenzyu/splitcast/pull/3)
is merged (2026-11-17); its final-head gate receipts remain scoped to the
generated-production test. No host activation or deployment approval follows.
