---
type: Source comparison
title: November 18 proposals and their acceptance boundaries
description: Find the tested revision for decoded freshness, Prepare failures and visible Studio readers.
status: draft
sources:
  - { id: S33, resource: ../evidence/dreaming-retrieval-20261118.md }
---
# Which revision answers this question?

Snapshot **2026-11-18 JST**. All three PRs were open drafts. Use this table to select a source before searching. A hosted run belongs to its PR event. Its individual jobs identify the checks that actually ran.

| Source | What to inspect | Scope of evidence |
| --- | --- | --- |
| [main `e92b5db`](https://github.com/tenzyu/splitcast/tree/e92b5db390835041b33de46618c49d5f98cad9cf) | #9 merged the finite local player, #15 added physical speech evidence, #16 merged the managed native PoC, #17 merged the convergence/wiki tooling. | [Two-host and viewer receipt](../evidence/pr16-managed-acceptance.md) identifies the tested binary at `d9b0948`. Web, TUI and CLI use the authenticated Agent. |
| [#18](https://github.com/tenzyu/splitcast/pull/18) on main | [Measurement oracle](https://github.com/tenzyu/splitcast/blob/5485ca89ca44259f10c7c2e1282841c35a7dce0f/tools/media_acceptance.py#L67-L122) | Production-only, generated ingress and real software OBS. [CI 44254307055](https://github.com/tenzyu/splitcast/actions/runs/44254307055) |
| [#19](https://github.com/tenzyu/splitcast/pull/19) on main | [Agent wire contract](https://github.com/tenzyu/splitcast/blob/51bd829d56169b930d60315d2682f0c4c6eec2f9/docs/studio-control-contract.md) | One EventSub subscription, bounded chat deltas independent of speech, generation-checked scene edit/readback/save, small Running preview. Local Agents and stand-ins. [CI 44268693685](https://github.com/tenzyu/splitcast/actions/runs/44268693685) |
| [#20](https://github.com/tenzyu/splitcast/pull/20) on `feat/studio-control-contract` (#19) | [Optional standalone Studio package](https://github.com/tenzyu/splitcast/blob/6fcec4a3a6882937bb8a7b6740b15e2affd0ea30/packages/splitcast-studio/README.md) | Slint client of #19. The Agent owns OBS, persistence and the session after the window closes. Private GUI fixtures. [CI 44272711325](https://github.com/tenzyu/splitcast/actions/runs/44272711325) |

A detached local combination applied #18, #19, then #20 to this main. Its Git tree and selected local software receipts could be inspected. Use the [snapshot and receipt identities](../evidence/dreaming-retrieval-20261118.md#source-snapshot) to distinguish it from each PR's tested build. Recheck [heads, base and merge state](update-protocol.md#when-a-proposal-moves) before relying on this table.

## Prepare returns 409

At #19 head `8557945`, the SIGTERM cleanup test failed **during Prepare**. The hosted upload recorded an I/O deadline after transmission began and an unknown outcome. It omitted the receiver worker log. The original hosted cause remains **unknown**. [Failed job 134632547127](https://github.com/tenzyu/splitcast/actions/runs/44254418762/job/134632547127)

The isolated test passed on the same revision. Concurrent local suites selected the same MediaMTX port. One worker logged `EADDRINUSE` and another fixture's listener could satisfy readiness. Deliberately occupying the receiver port reproduced the 409 text. #19 keeps bound, non-listening loopback sockets with `SO_REUSEADDR` through both Agents' teardown. The worker can listen on its assigned port. The regression checks exclusivity before Prepare and release after shutdown. Use the existing test and worker diagnostics before changing production timeouts. [Failed and passing experiments](https://github.com/tenzyu/splitcast/blob/51bd829d56169b930d60315d2682f0c4c6eec2f9/docs/verification.md#L48-L71) · [Reservation](https://github.com/tenzyu/splitcast/blob/51bd829d56169b930d60315d2682f0c4c6eec2f9/packages/splitcast-rs/tests/two_agents.rs#L321-L330) · [SIGTERM regression](https://github.com/tenzyu/splitcast/blob/51bd829d56169b930d60315d2682f0c4c6eec2f9/packages/splitcast-rs/tests/two_agents.rs#L777-L797)

## Chat or preview stalls after focus transfer

The old reader used keyboard focus to gate both reads. #20's `WindowState::viewable` uses a shown window and reported occlusion. Hidden panels issue zero corresponding polls. Winit on Wayland supplies no compositor occlusion event, so a window covered by another window cannot always be identified. [Old focus gate](https://github.com/tenzyu/splitcast/blob/04f60e32a8aaf6103a568000c231da53d35ffe5c/packages/splitcast-studio/src/ui.rs#L154-L173) · [Window events](https://github.com/tenzyu/splitcast/blob/6fcec4a3a6882937bb8a7b6740b15e2affd0ea30/packages/splitcast-studio/src/ui.rs#L73-L90)

### Visible reader scheduling

| Reader | When visible and unfocused | Scheduling rule at `6fcec4a3` |
| --- | --- | --- |
| Chat | Open panel and fresh Ready, Running or Degraded status | One cursor delta in flight. Drain additional pages immediately. A changed result resets the delay to 300 ms. An unchanged result doubles the delay up to 4 s. |
| Preview | Open editor and preview, fresh Running status | At most one snapshot per second. |

Pending Stop pauses both readers. Hidden panels make zero corresponding requests. [Reader conditions](https://github.com/tenzyu/splitcast/blob/6fcec4a3a6882937bb8a7b6740b15e2affd0ea30/packages/splitcast-studio/src/ui.rs#L165-L190) · [Read limits](https://github.com/tenzyu/splitcast/blob/6fcec4a3a6882937bb8a7b6740b15e2affd0ea30/packages/splitcast-studio/src/model.rs#L14-L17) · [Cursor backoff](https://github.com/tenzyu/splitcast/blob/6fcec4a3a6882937bb8a7b6740b15e2affd0ea30/packages/splitcast-studio/src/model.rs#L200-L211)

The private headless Sway/Pixman fixture disables Xwayland and transfers focus to a second native `xdg_toplevel` (foot) with both windows visible. Protocol traces show keyboard leave/enter. The old binary made zero chat/preview reads in six seconds. The candidate rendered injected row 129 with focus still on foot. Preview gaps were at least 1.1999 s in that sample. Hidden panels stayed quiet. Reuse the packaged before/after commands for this invariant. Earlier Xvfb tests cover GUI interactions and resource use. Physical osu!, real Twitch and a physical compositor have separate gates. [Focus assertions](https://github.com/tenzyu/splitcast/blob/6fcec4a3a6882937bb8a7b6740b15e2affd0ea30/packages/splitcast-studio/tests/wayland.py#L263-L280) · [Commands, results and limits](https://github.com/tenzyu/splitcast/blob/6fcec4a3a6882937bb8a7b6740b15e2affd0ea30/docs/verification.md#L202-L310)

The historical D02 GPUI preference preceded the measured package comparison. On Xvfb/llvmpipe GPUI exceeded the 80 MiB idle PSS target. The sampled Slint package used 20.888 MiB and zero idle redraws. This explains the candidate's choice for that workload. [Comparison](https://github.com/tenzyu/splitcast/blob/6fcec4a3a6882937bb8a7b6740b15e2affd0ea30/docs/verification.md#L118-L139) · [D02](decisions.md#d02-one-rust-command-and-validation-contract-serves-every-view)

## Bytes advance while the scene ages

#18 stamps generated pixels and decodes their identity from OBS screenshots. One monotonic clock bounds age from generation to observation. PTS intervals measure presentation cadence. Decoded marker progression exposes frozen pixels at regular PTS. A four-second backlog can preserve moving pixels and cadence while showing old content. Both controls had advancing ingress and output. Lower age bounds above three seconds proved staleness in this fixture. Overlap with the limit is inconclusive. This observer supplies a diagnosis outside the Agent watchdog. [Oracle and negative controls](https://github.com/tenzyu/splitcast/blob/5485ca89ca44259f10c7c2e1282841c35a7dce0f/docs/media-acceptance.md#L36-L85) · [Observed runs](https://github.com/tenzyu/splitcast/blob/5485ca89ca44259f10c7c2e1282841c35a7dce0f/docs/verification.md#L217-L255)

Normal Xvfb/llvmpipe, CPU decode and x264 at 720p30 measured **38/240 late renders (15.83%)** after warmup and **0/241 output skips**. The measurement matrix passed its software assertions. The normal render result exceeds the ≤1% performance target. The target on production hardware still lacks a valid window. Camera hashes establish changing pixels. Generated A/V transitions bound sampled fixture phase. The operator heard speech in a separate #16 physical session. Camera age and physical A/V offset still need independent timing. [Generated receipt](https://github.com/tenzyu/splitcast/blob/5485ca89ca44259f10c7c2e1282841c35a7dce0f/docs/verification.md#L219-L240) · [Hardware, clock and unique-marker criteria](https://github.com/tenzyu/splitcast/blob/5485ca89ca44259f10c7c2e1282841c35a7dce0f/docs/operations.md#L166-L228) · [Physical gates](open-risks.md)
