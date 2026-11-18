---
type: Working State
title: SplitCast session state
description: Current native candidate and live acceptance evidence.
status: draft
---
# Native two-host PoC · 2026-11-18 JST

## PR #16 stream-readiness follow-up

The review at `0823165` requires separately observable media integrity,
performance and cleanup. The follow-up changes the managed Start/Stop and
speech admission contracts. See the run record for source identity and actual
commands. [Review](https://github.com/tenzyu/splitcast/pull/16#pullrequestreview-5950165310) · [Verification](../verification.md)

| Acceptance gate | Evidence still required for the follow-up |
| --- | --- |
| Combined physical media | Final-source capture and production binaries running actual game video and game-only audio with the selected SoloCast through RNNoise, the selected camera inset, and the selected output. Independent decode checks for moving game content, camera freshness, isolated processed mic and A/V alignment. |
| Gameplay impact | Restart the same replay from its beginning in each OFF/ON/OFF phase. Compare p99 frame time against the 1 ms bound with sample counts and uncertainty. Observe Direct Scanout and game-only local audio. Record unmeasured input and audio dimensions. |
| Production OBS | Identify actual renderer, decoder and encoder. Measure at most 1% late renders after warmup and independently verify output integrity, an exit code of zero, and cleanup of owned processes and listeners. The historical 18.1% result remains a failure. A controlled generated-input comparison reproduced the recorded SIGSEGV with SIGTERM. SIGINT exited normally in its corresponding run, where 69/601 renders were late. Hardware-backed OBS remains blocked on a usable isolated NVIDIA display. [OBS diagnostic](../evidence/pr16-obs.md) |
| Managed Twitch output | At least five healthy minutes in bandwidth-test mode followed by verified Stop. The separately authorized bounded public smoke requires independently decoded viewer playback. Viewer-side confirmation is **BLOCKED** until an independent observation is available. |
| Live speech return | A live Twitch notification received on the selected account through EventSub, synthesized by production-local VOICEVOX and observed on the capture host's selected physical playback path during the integrated session. |

The [current runbook](../operations.md#native-poc-startup) defines Start
confirmation, timed Stop, and cleanup observations. The historical runs
below retain their own binary identities and observed scopes.

## Historical physical trials through review head `0823165`

The trial began from `origin/main` `3b2a051efc1c5f8c483d8f7f453d49f6216c7f2d`. Both hosts ran trial binary SHA-256 `c7ef4e4e897be3deaa9ba88b41b5e3bd19ca1bb46c7fde8f07616969d100920d`, built from the in-progress native candidate before the volume control and RNNoise-ready guard. The follow-up source changes have separate local verification. [Verification](../verification.md)

| Gate | Result | Observed boundary |
| --- | --- | --- |
| Authenticated game video and game audio → private OBS recording | **PASS** | Actual GSR 5.13.8 KMS helper, selected osu! PipeWire node, capture-initiated OpenSSH local forwards, MediaMTX, OBS 32.1.2. Recorded replay pixels were independently decoded and inspected. No other screen was identified in the inspected frame. |
| Recording bounds and independent decode | **PASS** | One 10.069 s, 7,673,426-byte MKV. FFmpeg decoded 603 H.264 video frames and AAC without errors. Encoded width was 1364 for the 1366×768 KMS output. All 603 video frame hashes differed. Video packet presentation intervals were at most 17 ms. Recording SHA-256 `1eea535a95cf2f51300835a303c370f464ec5ff514eddbbe2571ce039630894a`. |
| Production frame freshness and performance | **FAIL** | OBS reported 117 of 645 attempted frames late from rendering lag or stalls, 18.1%. Decoded changes and regular timestamps establish a moving and decodable recording. They do not measure capture-to-production frame age. |
| Selected mic / RNNoise / OBS | **PASS for acquisition and routing, BLOCKED for physical signal isolation** | HyperX SoloCast was an explicitly verified hardware ALSA source. Capture FFmpeg and production FFmpeg/RNNoise ran through the authenticated RTSP path. OBS received 985 packets from the private processed monitor. The live recording mixed mic and game tracks. The independent generated-input oracle verified known mic markers after a startup guard was added. A distinct acoustic physical marker and source-to-output phase check remain open. |
| Live Twitch → voice → physical playback | **PASS** | `channel.chat.message` v1 used Twitch-validated `tenzyudotcom` broadcaster and reader identity. One real notification, one admission, zero duplicates/drops/rejections. VOICEVOX OSS 0.25.2 synthesized it in 2.142 s locally. The operator heard the chat phrase on the capture host. No cross-host latency was inferred. |
| Playback fault recovery | **PASS, prior exact physical revision and current isolated tests** | The earlier two-host speech fault matrix includes lost SSH, Stop, missing sink, fresh epoch, exact player PCM and the 10 s truncated utterance. A 25 s budget delivered the full phrase. Current isolated tests exercise authenticated OpenSSH, epoch fencing, expiry and privately observed PipeWire output. [Original physical receipt](../evidence/real-voicevox-physical-return.md) |
| Capture host non-regression | **BLOCKED** | At sampled instants Direct Scanout and the same two game-to-physical-sink links held in 24/24, 52/52 and 24/24 OFF/ON/OFF samples. These were successive slices of one replay, with 7.568, 17.921 and 7.673 seconds of observation. Main-thread scheduling wait per slice (p95 0.87 / 9.67 / 2.30 µs) is a scheduler proxy. Matched replay repeats, all-frame gameplay timing, input measurements and physical A/V synchronization are open. |
| Shutdown and rollback | **PASS for release, FAIL for orderly process exit** | Owner-scoped media Stop was observed complete on both hosts. OBS finalized the file, then exited with SIGSEGV on release. An earlier teardown almost immediately stopped the peer after a 202, and capture needed the enclosing cgroup to SIGKILL residual work. The recording, trial core, temporary bearer/key files and scoped KMS wrapper were removed. Twitch revoked the trial access token. All trial listeners/cgroups were absent and both systems kept their prior active generations. |
| Physical camera → private OBS inset | **PASS for the independent camera slice** | The selected TOSHIBA V4L2 device provided 320×180/15 fps YUYV, encoded by one FFmpeg thread. SSH forwarded video-only H.264 to the production Agent's MediaMTX and OBS. Independent decode found the synthetic game marker and 197 distinct inset frame hashes in 197 frames. The 6.600 s, 4,985,958-byte recording was removed. The capture publisher ran in its own trial unit. The normal capture-Agent camera worker and live game were not exercised together. |
| Twitch production output | **PASS for bandwidth-test and public ingest** | Twitch accepted RTMPS in separate trials. Public mode was explicitly authorized, used only generated game and mic signals, and was Running five seconds after confirmed Start. Start request through completed Stop took 16.701 s. No independent viewer or VOD was decoded. The OBS profiles and all trial copies of the key were removed. |
| Web and TUI controls | **PASS** | Chromium used the actual page, authenticated same-origin Status and completed Stop, and showed empty browser storage. The terminal queried actual Agent status and declined public output without changing its session. The browser command-order fixture verified that Stop remains enabled during Start and owns the final displayed result. |

The capture host reported 1366×768 at 60.003 Hz and fullscreen Direct Scanout while osu! used the operator's 1280×720 internal configuration. The trial used the NixOS-generated `gsr-kms-server` wrapper in its own time-limited runtime directory. Only that helper received `CAP_SYS_ADMIN`. The existing wrapper symlink and system profile were checked before and after.

The next performance trial needs matched starts of the same playable replay in
OFF/ON/OFF conditions and an independent source/content marker. Measure
frame and scheduling tails on the same workload. For the next trial, require
an upper measured p99 gameplay frame-time increase of at most 1 ms, no
observed Direct Scanout or local game-audio interruption, and at most 1%
late production renders after warmup. Record uncertainty alongside frame-time
and input/audio observations. The production NVIDIA GPU is present and idle at
preflight. The software OBS run's 18.1% rendering lag warrants an isolated
hardware-backed OBS trial before repeating 60 fps acceptance. The source
and runtime remain gated from autostart.

The physical camera and bandwidth-test receipts both use binary SHA-256
`a67e43c757c32bc9341ae6007220b276a0a9a7d3d1110ec385305d9518ee9620`
(source tree `067642b7d1e7cf285ed9e93a47df73435bb50a3343c625f0c84f672d90954226`
over `17594cf`). The public trial uses
`725b4ae60255af419c499f0a8056edcc1950ba8bfb4c5c68d82cf0c6c97232e6`,
with the public OBS process cap and shorter Stop budget. Private receipts
are in `artifacts/mission/camera-attempt-1.json`,
`twitch-ingest-attempt-1.json`, and `twitch-public-attempt-1.json`.

The follow-up trials did not activate a system generation. The capture system
and wrapper matched their original snapshots. The production host was on
generation 64 during the follow-up. Its activation timestamp precedes the
first follow-up run. Generation 63 from the earlier trial still has a system
profile link and recovery entry point. The comparison difference was retained
in the cleanup receipt, and the follow-up left generation 64 in place.

[Decisions](decisions.md) · [Original baseline](https://github.com/tenzyu/splitcast/blob/3b2a051efc1c5f8c483d8f7f453d49f6216c7f2d/docs/memory/session-state.md) · [Evidence index](evidence.md)
