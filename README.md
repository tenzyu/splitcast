# SplitCast

SplitCast runs OBS production on a second Linux host while osu! renders, handles input and plays its game audio on the capture host.

```text
osu! Direct Scanout + selected game audio → GSR/H.264/AAC → SSH → MediaMTX → OBS
Selected ALSA microphone → FFmpeg/Opus → SSH → FFmpeg/RNNoise → private Pulse → OBS
Selected V4L2 camera → FFmpeg/H.264 → OBS picture-in-picture
OBS → private recording or Twitch RTMPS
Twitch EventSub → VOICEVOX → owner-scoped SSH return → selected PipeWire playback
```

The two Agents arrange the lifecycle around these native tools. Media Prepare establishes the authenticated session and its private receiver. Start owns video acquisition, the mic, the optional camera, private PulseAudio, OBS and one operator-managed output. Speech-only Prepare opens a single control forward.

Use the CLI, `splitcast --config FILE tui`, or the Web client at the Agent's loopback control address. All three use the same authenticated commands. Private recording is the default. Twitch bandwidth test and public broadcast are separate output selections. A public Start requires production opt-in, a prepared media session, explicit confirmation and an operator-selected duration. [Operation and configuration](docs/operations.md#twitch-output)

**Status.** Earlier two-host trials produced decodable osu! footage, played a Twitch chat message through VOICEVOX on the capture host, and exercised the camera and short Twitch ingest in separate slices. The existing PR tracks clean OBS exit, integrated physical media, matched gameplay performance, sustained bandwidth testing and independent viewer playback as separate acceptance gates. [Current evidence](docs/memory/session-state.md)

## Build and verify

```sh
nix build .#default
./result/bin/splitcast selfcheck
./result/bin/splitcast validate --config packages/splitcast-rs/example.toml
./result/bin/splitcast validate --config packages/splitcast-rs/example-production.toml
nix develop -c cargo run -p splitcast-xtask -- verify
```

The flake provides a native x86_64-linux package and optional NixOS and Home Manager modules. The Agent has no autostart target, and the runtime gate defaults to false. Both role examples leave physical inputs unselected. Keep credential files outside the repository and Nix store. [Capture example](packages/splitcast-rs/example.toml) · [Production example](packages/splitcast-rs/example-production.toml) · [Verification](docs/verification.md)

Capture source, microphone, return sink and return stream gain are selected in the host configuration. Set `audio.tts_volume_percent` from 0 to 100 and restart the capture Agent after Stop completes. The setting applies to the owned `pw-play` stream.

## Operate a session

After staging both roles and starting their Agents, use the capture-side configuration.

```sh
splitcast --config capture.toml prepare
splitcast --config capture.toml start --output recording --duration-seconds 20
splitcast --config capture.toml status
splitcast --config capture.toml stop
```

`running` shows Agent-observed output and media progress. Use decoded output to check content and timing. A Stop request can return while cleanup is still in progress. Proceed after the matching completion is observed and check `obs_shutdown` on production for the output stop and process exit receipts. [Startup and rollback](docs/operations.md#native-poc-startup)

## Implementation

- [Native Agent](packages/splitcast-rs) owns session leases, bounded worker cleanup and control.
- [NixOS and Home Manager modules](nix/modules) supply pinned runtime tools and keep activation explicit.
- [Versioned Wiki](docs/memory/index.md) indexes the accepted behavior, counters and unresolved gates.
