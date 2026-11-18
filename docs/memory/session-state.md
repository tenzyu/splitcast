---
type: Working State
title: SplitCast session state
description: Current native candidate and live acceptance evidence.
status: draft
---
# Native two-host PoC · 2026-11-18 JST

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

The capture host reported 1366×768 at 60.003 Hz and fullscreen Direct Scanout while osu! used the operator's 1280×720 internal configuration. The trial used the NixOS-generated `gsr-kms-server` wrapper in its own time-limited runtime directory. Only that helper received `CAP_SYS_ADMIN`. The existing wrapper symlink and system profile were checked before and after.

The next performance trial needs the same playable replay section in all three conditions, an agreed error budget and an independent source/content marker. Evaluate a less expensive production OBS configuration before repeating 60 fps acceptance. The source and runtime remain gated from autostart.

[Decisions](decisions.md) · [Original baseline](https://github.com/tenzyu/splitcast/blob/3b2a051efc1c5f8c483d8f7f453d49f6216c7f2d/docs/memory/session-state.md) · [Evidence index](evidence.md)
