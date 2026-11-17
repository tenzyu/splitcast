---
type: Working State
title: SplitCast session state
description: Current implementation and blockers; dated runs stay in evidence.
status: draft
sources:
  - { id: main, resource: https://github.com/tenzyu/splitcast/commit/ed7fe60e8efd0dea454346b13f38a680096fb60e }
  - { id: ci, resource: https://github.com/tenzyu/splitcast/actions/runs/44097472963 }
---
# Current state — 2026-11-17

| Boundary | Verified for specified scope | Not established |
| --- | --- | --- |
| Repository | PR #2–#9 merged, main `ed7fe60`; 4/4 hosted CI jobs successful | Deployment, physical health, complete PoC |
| Control | Owner-scoped lifecycle, urgent Stop admission and async-`202 requested` test correction | Universal cleanup deadline, crash escape, durable outcomes |
| Media | Generated *production-only* Agent → MediaMTX → RNNoise/private Pulse → OBS → decoded output; negative freshness controls | Capture Agent/SSH real media, live freshness enforcement, clock relationship, competitive non-regression |
| Speech | `poc-play` WAV validation, explicit target, bounded local native-player completion; independent production-local VOICEVOX synthesis | Twitch `channel.chat.message` ingestion, authenticated return, capture-host listening |
| Capture | Read-only inventory identified GSR/helper absent and cached FFmpeg lacking Pulse input | Usable launch and KMS privilege, real game/mic media, physical performance |
| Release | Earlier signed cache delivery and main hosted checks | Manual release run `44097570226` was still in progress at S26 inspection; do not presume its outcome |

The 2026-11-18 fixture validation follow-up on branch `fix/poc-offline-acceptance` is recorded in [S27/E22](evidence.md). It covers chunk order at the player boundary. Live speech acceptance remains open.

## Next step and stop conditions

Follow [PoC handoff](../handoff/poc-20261117.md) and [S25 convergence](../evidence/poc-convergence-20261117.md), corrected for already-merged PR #9 by [S26](../evidence/dreaming-main-20261117.md). Select existing channel and OAuth **path**, or existing receiver output; never request raw credentials. Production WSS/Helix/subscription was granted; no new OAuth, chat post or public stream. Capture SSH remains read-only; remote execution, installation and generation switch need independent approval. Do not restart closed CI/cache, PR-conflict or environment-gap tasks.

Three independent acceptance axes: (1) fixed-workload all-frame gameplay OFF/ON/OFF tails and Direct Scanout, (2) decoded media freshness, cadence and sync with clock uncertainty, (3) real event → production synthesis → remote playback timing, audibility and fault isolation. Bytes, nominal FPS, a green CI or a synthetic recording do not substitute for those observations.

## Preserved counterexamples

- Prior Prepare/reset failures were not diagnosed; passes at different source and under probe concurrency do not prove a fix.
- `Stop: requested` means accepted intent, not confirmed owner-specific completion.
- Source-ingress backlog is not media-network congestion; advancing bytes can conceal frozen pictures.
- 30-fps headers and aggregate microphone-presence ratios hid VFR frames and full 100-ms PCM dropouts. Negative controls use decoded PTS/window continuity.
- Local process completion does not prove listening, and independent VOICEVOX/NVENC tests do not prove the joined path.

[Decision IDs](decisions.md) · [Evidence IDs](evidence.md) · [Risk IDs](open-risks.md) · [Archived original session capsule](https://github.com/tenzyu/splitcast/blob/archive/pre-dreaming-20261117/docs/memory/session-state.md).
