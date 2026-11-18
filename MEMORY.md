# SplitCast memory

Preserve osu! gameplay and its local audio while offloading selected game A/V, the HyperX trial microphone through RNNoise, private OBS recording and VOICEVOX playback. Capture and production are separate roles. SplitCast coordinates native workers and authenticated sessions.

**Current PoC.** On 2026-11-18 JST the two hosts recorded real osu!, camera and SoloCast/RNNoise. Independent decoding confirmed the public Twitch archive, and both media sessions verified Stop and OBS exit. The operator heard an actual Twitch chat message through VOICEVOX in a separate speech session. NVIDIA OBS, gameplay frame times and physical A/V timing remain open. The original render stalls and SIGSEGV are retained. [Current state and source-bound evidence](docs/memory/session-state.md)

The current scope includes optional camera, Web and TUI controls, and Twitch RTMPS output. Private recording remains the default. [Operation and configuration](docs/operations.md)

Read [the index](docs/memory/index.md), then the relevant concept and source. [PR #15 physical speech evidence](docs/evidence/real-voicevox-physical-return.md) preserves the 10-second expiry failure and successful 25-second return. Dated handoffs describe their original permissions and results. Current operator instructions govern new host effects.

The Nix runtime gate is false and the service has no autostart target. Run `nix develop -c cargo run -p splitcast-xtask -- verify` unless the pinned direnv shell is already loaded. Exact commands and failures belong in [the verification record](docs/verification.md).
