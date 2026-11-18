# SplitCast

SplitCast runs private OBS production on a second Linux host while osu! renders, handles input and plays its game audio on the capture host.

```text
osu! Direct Scanout + selected game audio → GSR/H.264/AAC → SSH → MediaMTX → OBS
Selected ALSA microphone → FFmpeg/Opus → SSH → FFmpeg/RNNoise → private Pulse → OBS
Twitch EventSub → VOICEVOX → owner-scoped SSH return → selected PipeWire playback
```

The native CLI and two Agents arrange the lifecycle around these native tools. Prepare establishes the authenticated session. Media Prepare includes the private media receiver. Start owns video acquisition, the mic, private PulseAudio, OBS and a bounded recording. Speech-only Prepare opens a single control forward.

**Status.** The live two-host trial produced a decodable osu! recording and played an actual Twitch chat message on the capture host. Recording cadence under production rendering load, independently identified physical mic output, gameplay non-regression and clean OBS process exit remain acceptance gates. [Current evidence](docs/memory/session-state.md)

## Build and verify

```sh
nix build .#default
./result/bin/splitcast selfcheck
./result/bin/splitcast validate --config packages/splitcast-rs/example.toml
nix develop -c cargo run -p splitcast-xtask -- verify
```

The flake provides a native x86_64-linux package and optional NixOS and Home Manager modules. The agent has no autostart target, and the runtime gate defaults to false. The example leaves physical inputs unselected. [Operations](docs/operations.md) · [Verification](docs/verification.md)

Capture source, microphone, return sink and return stream gain are selected in the host configuration. Set `audio.tts_volume_percent` from 0 to 100 and restart the capture Agent after Stop completes. The setting applies to the owned `pw-play` stream.

## Implementation

- [Native Agent](packages/splitcast-rs) owns session leases, bounded worker cleanup and control.
- [NixOS and Home Manager modules](nix/modules) supply pinned runtime tools and keep activation explicit.
- [Versioned Wiki](docs/memory/index.md) indexes the accepted behavior, counters and unresolved gates.
