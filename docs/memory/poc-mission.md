# SplitCast PoC mission

Updated 2026-11-17. **User goal, not deployment authorization.** Start the next
environment-gap session at [the gap handoff](environment-gap.md), not an old
release checklist. This page integrates S02–S07, S12, S20 and S21 from
[the source register](evidence.md); historical records remain unchanged.

## Why SplitCast exists

The capture host must prioritize osu!lazer's Input, Audio, Update, Draw and
display. Move production work to the production host while minimizing added
CPU/GPU work, memory traffic, heat, power and scheduling interference on the
capture host. Failure of streaming, networking or the production host must not
block, restart, reschedule or otherwise impair gameplay or local game audio.

**A separate required return path** sends speech or other audio computed on the
production host back to local playback on the capture host. This is a renewed
user requirement in S21, not an implemented or accepted feature. A synthesis
component and a configurable sink do not establish delivery or playback.

Ultra Low Latency and Ultra Lightweight are goals. No numeric target or achieved
physical result is adopted here. Three budgets must stay separate:

| Budget | Question | Evidence currently available |
| --- | --- | --- |
| Gameplay interference | What changes in input/audio/frame tails and shared-resource pressure relative to the game-only baseline? | Historical scanout/graph observations E01; no accepted capture-on competitive non-inferiority result. |
| Media freshness and continuity | How old is decoded content, and does it arrive with usable cadence and audio? | E16 is production-only synthetic ingress, local scene age and sampled game A/V, not live freshness enforcement or a deployment latency budget. S22 tightens the oracle; see [the follow-up](../evidence/pr3-review-20261117.md). |
| Returned-audio latency and isolation | How long from a production-generated utterance/event to capture-host playback, and what happens on failure? | Not accepted. No transport, TTS-first sequence, expiry budget or latency number is selected by S21. |

## Responsibility and dependency boundaries

```text
capture host                              production host
osu! input/update/draw/display             receive/decode -> compose -> record/publish
local game audio (independent)             microphone noise removal
  | selected pixels/game audio/mic ------> speech synthesis and other production work
  | minimal acquisition/encode/send
local optional playback <---------------- computed speech/other returned audio
```

- Unavoidable capture-host work: acquire the selected output, tap authorized
  game audio and optional microphone, perform the necessary encoding/transport,
  and receive/play returned audio. Offloading OBS does **not** remove acquisition,
  encode, synchronization, GPU or memory costs. Which local encoding is necessary
  depends on the selected transport and environment; no zero-cost claim.
- Production-host work: receive-side processing, noise removal, composition,
  recording/publication and synthesis. Keep production failure downstream of
  gameplay. Returned audio must not replace the local game playback path.
- SplitCast should be a thin integration layer. Reuse existing software unless
  an identified, measured bottleneck cannot be removed through configuration or
  a narrow integration. Do not duplicate RTMP, OBS, HTTP, TTS or a general
  distributed control platform on the strength of a hypothetical bottleneck.
- Separate hot media paths from infrequent configuration/control. D02's shared
  typed command/validation contract remains useful; it does not require every
  PoC to add clients, GUI features, route graphs or persistent distributed state.

## Current implementation is not the target's proof

Source checkpoint: PR #3 original head `8486e819ed6d95329a3bcfede4593a831747b300`;
media execution belongs to `42a64e41990eb3a411dcd68e3412ccceac574112`.
Current follow-up changes are identified in [S22's receipt](../evidence/pr3-review-20261117.md).

| Source fact at that checkpoint | What it means for the goal |
| --- | --- |
| [`agent.rs::prepare`](../../packages/splitcast-rs/src/agent.rs) refuses enabled EventSub, TTS or remote TTS mixing. Speech commands do not complete delivery/playback. | The required return path is missing, not optional merely because the current candidate refuses it. Preserve refusal until implemented and accepted separately. |
| [`media.rs::game/mic/tunnel`](../../packages/splitcast-rs/src/media.rs) launches GSR H.264/AAC, FFmpeg Opus, and loopback SSH local forwards for control/RTMP/RTSP. | The capture host still encodes; TCP/SSH freshness, overhead and cross-host clocks remain unaccepted. Current forwards are not a returned-audio implementation. |
| [`obs.rs`](../../packages/splitcast-rs/src/obs.rs) configures OBS input decoding, composition and final encoding; [`media.rs::rnnoise`](../../packages/splitcast-rs/src/media.rs) feeds private production Pulse. | Existing dependencies implement the heavy production path. The software video mode is for the device-free fixture; hardware defaults do not establish competitive non-regression. |
| [`tools/media_acceptance.py`](../../tools/media_acceptance.py) generates ingress on the production side. | It proves actual production integration only. It bypasses capture Agent, GSR acquisition and SSH. A stronger oracle does not expand that scope. |

## Candidate comparison, not a new architecture decision

| Candidate/inventory | Reason to retain or consider | Missing evidence / cost |
| --- | --- | --- |
| Existing GSR → SSH/RTMP/RTSP → MediaMTX → OBS integration | Existing code and production-only receipts; no game injection. | Capture-side encode/load, privilege boundary, TCP head-of-line blocking, redundant stages and real two-host freshness. Not an optimality result. |
| Configure/reuse a narrower existing capture, transport or audio-return facility | Can keep SplitCast thin; evaluate before custom infrastructure. | Environment inventory and capability/security/failure fit first. No particular protocol or TTS engine approved. |
| Secure datagram transport or minimal custom hot-path component | S04 proposed a comparison if TCP or acquisition costs are a proven limiter. | A proposal only; clock, loss, queue, security and resource contracts would need evidence. Do not build before establishing the bottleneck. |
| Game-side capture hook or silent renderer/portal substitution | Historically considered, then rejected for the working scenario. | S02 records an EGL-export-path crash, not a proven Mesa root cause. Portal may break scanout; neither is a transparent fallback. |

## What the history teaches

- S02/S03 distinguish game hooks, portal and KMS. Moving OBS alone cannot remove
  capture-side interference. S04 is explicitly a **proposed** target, not an
  approved transport/broker implementation or current host state.
- S12's reversed-direction synthetic recoveries cannot prove the normal route;
  its normal-direction pass leaked SSH workers. E05's later 600-second transport
  success used neither Agent nor OBS. Keep direction, owner and cleanup attached.
- S06/D05 retired Python and refused incomplete live speech; this did not retire
  the user's bidirectional requirement. Historical rollout permissions expired
  with their task. D15 below revisits product scope without enabling anything.
- S20 disproved software-mode device isolation and phase-fit timing. Current
  review found header FPS and overall audio-presence ratios insufficient. Test
  the observed output, not the control acknowledgement or a nominal setting.

See [D01/D02/D06/D14 and dated D15/D16](decisions.md),
[evidence scopes](evidence.md), [open risks](open-risks.md) and
[the unique next-session entry](environment-gap.md). Private host mappings,
access details and raw logs remain outside the public Wiki.
