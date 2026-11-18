# SplitCast memory

Preserve osu! gameplay and its local audio while offloading selected game A/V, the HyperX trial microphone through RNNoise, private OBS recording and VOICEVOX playback. Capture and production are separate roles. SplitCast coordinates native workers and authenticated sessions.

**Current PoC.** The 2026-11-18 JST trial recorded real osu! through GSR, SSH, MediaMTX and OBS. An actual Twitch chat message triggered VOICEVOX and the operator heard the return. Rendering stalls, shutdown failure and the incomplete workload comparison are recorded alongside those passes. [Current state and source-bound evidence](docs/memory/session-state.md)

Read [the index](docs/memory/index.md), then the relevant concept and source. [PR #15 physical speech evidence](docs/evidence/real-voicevox-physical-return.md) preserves the 10-second expiry failure and successful 25-second return. Dated handoffs describe their original permissions and results. Current operator instructions govern new host effects.

The Nix runtime gate is false and the service has no autostart target. Run `nix develop -c cargo run -p splitcast-xtask -- verify` unless the pinned direnv shell is already loaded. Exact commands and failures belong in [the verification record](docs/verification.md).
