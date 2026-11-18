# SplitCast

**Keep gameplay on one Linux host. Run OBS production on another.**

SplitCast is an experimental, Linux-native two-host streaming system for
performance-sensitive gaming, including osu!. The capture host keeps the game,
its input, and local game audio. A second host handles OBS composition, private
recording, and optional Twitch output.

Instead of replacing OBS or building a new media stack, SplitCast coordinates
native Linux tools with authenticated, owner-scoped sessions.

> **Project status:** Working two-host media and broadcast paths have been
> demonstrated, but this is still a proof of concept—not a production-ready
> streaming appliance. Gameplay performance, end-to-end synchronization, and
> some hardware/recovery gates remain unaccepted.
> [Evidence and limitations](#project-status-and-limitations)

## Why two hosts?

Running capture, composition, and broadcast production on the gaming machine
can compete with the game for CPU, GPU, and scheduling time. SplitCast moves
**OBS production** to a second Linux host while retaining the original game's
input and audio routing on the capture host.

The capture host **still performs video acquisition and encoding**, and
transport has its own cost. Preserving Direct Scanout and game responsiveness
is a design goal—not a claim of zero overhead or proven latency improvement.
See [physical acceptance evidence](docs/evidence/pr16-managed-acceptance.md)
for what was actually observed.

## What SplitCast does

- **Game capture:** selected display/game video through GPU Screen Recorder
  (GSR), with selected game-only audio; no automatic whole-desktop audio mix.
- **Audio production:** selected ALSA microphone transported for production-side
  FFmpeg/RNNoise processing, with separate game and microphone tracks in
  supported private recordings.
- **Optional camera:** a selected V4L2 source on either host as an OBS
  picture-in-picture input.
- **Outputs:** bounded private OBS recordings, a Twitch bandwidth-test mode,
  or an explicitly authorized public Twitch broadcast.
- **Chat and speech:** Twitch EventSub messages, VOICEVOX synthesis, and
  owner-scoped return playback to a selected PipeWire sink.
- **Controls:** authenticated CLI, terminal UI, loopback Web client, and the
  optional native [SplitCast Studio](packages/splitcast-studio/README.md)
  for chat, preview, and scene editing.

These capabilities have **different acceptance boundaries**. For example,
physical speech return was demonstrated in a separate speech-only trial; that
does not establish uninterrupted speech return during a full public game
broadcast. See [current evidence](docs/memory/session-state.md).

## How it works

```text
Capture host                                      Production host
------------                                      ---------------
osu! + selected game audio -> GSR/H.264/AAC --SSH--> MediaMTX --> OBS
selected ALSA microphone -> FFmpeg/Opus ------SSH--> MediaMTX --> FFmpeg/RNNoise --> OBS
optional V4L2 camera -> FFmpeg/H.264 ---------SSH--> MediaMTX --> OBS inset
                                                    OBS --> private MKV / Twitch RTMPS

selected PipeWire playback <-- owner-scoped SSH <-- VOICEVOX <-- Twitch EventSub
```

The diagram shows a capture-host camera. A production-host camera publishes
directly to the local MediaMTX receiver.

The capture and production Agents coordinate preparation, session ownership,
start/stop, and resource cleanup. Media preparation establishes an authenticated
session and private receiver; Start owns the selected media workers and one
operator-managed output. Stop is an **asynchronous operation**: a request being
accepted is not proof that both Agents and OBS have finished releasing resources.

See [operations and rollback](docs/operations.md) and the
[Studio control contract](docs/studio-control-contract.md) for details.

## Requirements

- **Platform:** the published Nix flake builds for `x86_64-linux`. Two Linux
  hosts are required for the intended capture/production architecture.
- **Capture host:** media sessions require an explicitly selected capture
  target, game-only audio source, and hardware ALSA microphone, plus GSR/KMS
  access and permissions appropriate to the host. Select a PipeWire playback
  sink if you enable speech return.
- **Production host:** OBS and the production-side media tools, with a working
  display/rendering environment for the selected output path.
- **Transport and secrets:** authenticated SSH connectivity and independently
  provisioned control/OBS credentials. Keep secrets out of Git and the Nix
  store; do not assume an example SSH alias or device name exists.

The NixOS and Home Manager modules are **opt-in**. They do not activate a
working production service just because a package was built; the runtime gate
is disabled by default. Hardware capture, privileged helpers, host
configuration changes, and public broadcasts require separate operator
review. See [Nix integration](docs/nix-integration.md) and
[the operational runbook](docs/operations.md).

## Quickstart: build and check locally

From a checkout of this repository, with Nix available:

```sh
nix build .#default
./result/bin/splitcast selfcheck
./result/bin/splitcast validate --config packages/splitcast-rs/example.toml
./result/bin/splitcast validate --config packages/splitcast-rs/example-production.toml
```

These checks do **not** start an Agent, access hardware, connect to a peer, or
broadcast. Both examples intentionally leave the physical capture and audio
sources unselected. Passing validation proves the example configuration is
well-formed, **not** that a live streaming environment is ready.

To inspect missing local prerequisites without contacting a peer:

```sh
./result/bin/splitcast doctor --offline --config packages/splitcast-rs/example.toml
```

Doctor checks configured source selections and local tool availability. The
example leaves the capture target, game audio, and microphone unselected.
Configure each host before attempting an actual session.

[Capture configuration](packages/splitcast-rs/example.toml) ·
[Production configuration](packages/splitcast-rs/example-production.toml) ·
[Deployment and prerequisites](docs/nix-integration.md)

## Run a bounded private session

**Only after** both Agents are configured, their authentication and SSH
transport are verified, required devices are explicitly selected, and the
operator has started the Agents on their respective hosts, prepare and start
the recording from the capture host. Replace `capture.toml` with the path to
its configuration. Set the production unit's recording file size limit
before Start. See [Native PoC startup](docs/operations.md#native-poc-startup).

```sh
splitcast --config capture.toml prepare
splitcast --config capture.toml start --output recording --duration-seconds 20
splitcast --config capture.toml status
```

The duration limit schedules Stop on production. To end the recording early,
run `splitcast --config capture.toml stop` on the capture host. Check for
matching Stop completion and the production `obs_shutdown` receipt, then
independently decode the recording to verify its contents.

Public Twitch output is a **separate, opt-in mode**, not part of this
Quickstart. It requires explicit production configuration, confirmation, and a
finite duration. See [Twitch output](docs/operations.md#twitch-output) and
[stop/rollback](docs/operations.md#stop-and-rollback).

For interactive operation, use `splitcast --config FILE tui`, the authenticated
loopback Web client, or [SplitCast Studio](packages/splitcast-studio/README.md).
All of them act through the Agents; closing a client does not stop a session.

## Project status and limitations

**Evidence snapshot: 2026-11-18 (JST).** The physical two-host trial at
`d9b0948` produced an independently decoded private OBS recording of moving
osu! video, camera video, game audio, and a processed microphone track.
Streamlink downloaded a segment from the separately authorized public
output's Twitch archive, and FFmpeg decoded it to the end. OBS exited
normally in both trials. A 302-second Twitch bandwidth test used
**generated media**.

**Still open:** matched gameplay p99 and input-latency non-regression; physical
A/V alignment and camera/frame age; acceptance of the intended NVIDIA
render/encode/decode path; integrated on-air chat-to-speech playback; and
recovery from certain production-host kernel-stuck workers. An earlier
software-rendered trial recorded **117 late OBS renders out of 645 attempts**;
subsequent successful output does not by itself prove the performance gate.

The [current session state](docs/memory/session-state.md),
[open-risk register](docs/memory/open-risks.md), and
[source-bound physical receipts](docs/evidence/pr16-managed-acceptance.md)
are authoritative for the exact revision, test scope, failures, and any later
corrections. A decodable file or passing CI job is not global acceptance.

## Documentation and development

| Need | Reference |
| --- | --- |
| Operate, stop, recover, or test a session | [Operational runbook](docs/operations.md) |
| Configure NixOS/Home Manager | [Nix integration](docs/nix-integration.md) |
| Configure capture and production | [Capture example](packages/splitcast-rs/example.toml) / [Production example](packages/splitcast-rs/example-production.toml) |
| Use the native desktop client | [SplitCast Studio](packages/splitcast-studio/README.md) |
| Read the control/scene API contract | [Studio control contract](docs/studio-control-contract.md) |
| Reproduce checks and inspect their limits | [Verification](docs/verification.md) |
| Navigate design decisions and remaining risks | [Versioned knowledge index](docs/memory/index.md) |

The Rust Agent lives in [`packages/splitcast-rs`](packages/splitcast-rs);
[Nix modules](nix/modules) integrate the pinned runtime tools. To run the
workspace verification suite:

```sh
nix develop -c cargo run -p splitcast-xtask -- verify
```

This runs local source checks and tests, **not** physical two-host acceptance.
Report issues or propose changes with the exact revision, environment, steps
to reproduce, expected/observed result, and any independently captured
evidence.

SplitCast is distributed under the [MIT License](LICENSE).
