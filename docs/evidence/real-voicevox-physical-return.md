# Real VOICEVOX and physical return acceptance checkpoint

Recorded 2026-11-18 JST on the production and capture hosts. **Physical return is BLOCKED.** Production-local synthesis and a loopback Agent authorization preflight passed. Capture-host observation used the existing read-only SSH route and the already active PipeWire graph.

## Source and prior software acceptance

Tested `main` at `6a503e92b9494b70514a2c5664d723f533b779b7`, with tree `dc160cda8c95004ee337019e29c71568d82f3ddd`. Its first parent is PR #14's merge, `cacde5b341a6a31c1d6f2d6430e0394deb4e505a`. The PR head `fcbf93e1ab27ef6bd784e0f94f10a3e606b04025` has the same tree and passed all four hosted CI jobs. The second parent preserves the local PoC history. [PR #14](https://github.com/tenzyu/splitcast/pull/14) · [Hosted CI](https://github.com/tenzyu/splitcast/actions/runs/44201103624)

The prepared protocol uses bearer-authenticated HTTP on an SSH local forward initiated by capture toward production. Capture polls production using its held owner. Production removes a ready item before transmitting its reply. Capture checks the epoch, sequence and remaining expiry before feeding PCM16 into `pw-play`. Capture configuration selects the node and the player requests no fallback or reconnect. The prior isolated acceptance exercised these mechanisms using controlled WAV responses, real OpenSSH and private PipeWire nodes on one kernel. [Prepared audio return evidence](audio-return.md)

## New host observations

| Requirement | Result | Evidence |
| --- | --- | --- |
| A. Real synthesis | **PASS** | The pinned VOICEVOX ENGINE OSS 0.25.2 process owned its loopback listener. `/version` returned `latest` and `/speakers` identified style ID 3 as ずんだもん・ノーマル. `/audio_query` and `/synthesis` generated `試験です。`. The 46,124-byte WAV contained 23,040 mono PCM16 samples at 24 kHz (0.960 s). FFmpeg independently decoded 46,080 bytes that matched the PCM extracted with Python `wave`, with no resampling or channel change. |
| B. Cross-host delivery | **BLOCKED** | The production-host Agent built from the tested tree listened on loopback. A valid bearer obtained v1 status (200), a missing bearer got 403, and ownerless pull, pull without a session, and `say` before Prepare got 409. The read-only SSH login reached the distinct capture host in the production-to-capture direction. The capture-initiated return forward, active ownership, expiry and cross-host content match remain untested. |
| C. Physical output | **BLOCKED** | Read-only PipeWire metadata identified a candidate ALSA output while PipeWire and WirePlumber were active. Returned signal at that sink and operator-confirmed listening remain unobserved. |
| D. Failure isolation | **BLOCKED** | Owned-forward interruption, unavailable target, Stop during a pending return, fresh-session recovery and uninterrupted local game audio remain untested on the two hosts. No identifiable game output stream was present at the inventory instant. |
| E. Evidence quality | **PASS** | The receipt distinguishes source behavior, isolated software acceptance, and live observations. Competitive gameplay impact and acoustic or cross-host latency remain unmeasured. |

PCM SHA-256 `137b8005b3d0e087acacc31c49bd2795c29be06f18b71ca07529c6674aa8e4b9` identifies the newly synthesized production-local PCM. The trial kept audio in memory. A future cross-host check must correlate PCM from the same synthesis request with native player input, then account for mixing, channel conversion or resampling in any sink measurement.

## Verification and cleanup

| Command or check | Result |
| --- | --- |
| `curl --noproxy '*' --silent --show-error --fail --max-time 2 http://127.0.0.1:50021/version` | Exit 7 before the trial. The configured example port refused the connection. |
| `nix path-info --offline .#splitcast-voicevox` | Exit 1 before the build. The pinned output was absent from the local store. |
| `timeout 360s nix build --no-link --print-out-paths --no-write-lock-file --max-jobs 1 --cores 2 .#splitcast-voicevox` | Exit 0. Restored the pinned engine output for a transient test. |
| `timeout 150s direnv exec . cargo build -p splitcast-rs-core --bin splitcast --locked --offline` | Exit 101. `splitcast` is the wrapped package alias. |
| `timeout 150s direnv exec . cargo build -p splitcast-rs-core --bin splitcast-rs-core --locked --offline` | Exit 0. A separate loopback preflight exercised the native binary and confirmed the listener's PID. Production `doctor --offline` returned `ok=false` with `mediamtx` missing from that environment. |
| `git ls-remote git@github.com:tenzyu/splitcast.wiki.git HEAD` and an authenticated HTTPS check | Both exited 128 with `Repository not found`. The durable summary is in the versioned Wiki under `docs/memory/`. |

The production probes used uniquely named transient user units and private temporary directories. Both units stopped with exit 0 and ended inactive with MainPID 0. Their cgroups, listeners, and temporary directories were released, including the trial-only bearer. The scoped inventory changed no capture routing or game settings. The full verifier and physical faults were unrun for this checkpoint. The operator-local receipt retains exact probe invocations, cleanup checks and failures.

## Remaining gates

Capture Prepare requires `ssh`, `gpu-screen-recorder`, `ffmpeg`, `ffprobe`, `pactl` and `pw-play` on its PATH. The inventory found `pw-play`, but found neither a SplitCast agent nor GSR in the scoped PATH and cache search. FFmpeg and FFprobe were present in cache outside PATH, and `pactl` was absent from PATH. The packaged runtime wraps its dependencies. Staging that runtime for capture and running it require a scoped execution path. Production Prepare also starts MediaMTX, private PulseAudio and OBS. A private production test needs appropriate local dependencies and the loopback-patched OBS build.

Resume with an approved finite capture launch, verified capture-to-production SSH forwarding credentials, and operator confirmation of the selected output. Use Prepare without Start, with the same return contract and expiry on both roles. An unavailable-sink probe can select an absent explicit node while leaving the real sink and game route intact. Test expiry, transport loss, Stop and recovery with a fresh session and new utterance. Clean up the owned cgroups, forwarded sockets, players and links before declaring each result. [Operations runbook](../operations.md)
