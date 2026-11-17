# SplitCast

**Two-host media production for Linux gameplay, without putting the game under the production host's control.**

SplitCast is an experimental Rust controller for a setup in which one Linux host captures gameplay and another handles media production. The capture host keeps running the game, rendering its display, and playing game audio locally. SplitCast coordinates separate native media workers and their control lifecycle; it does not inject code into the game.

The goal is to move composition, recording, streaming, and other production work away from performance-sensitive gameplay **without making gameplay depend on the production host**. This is an architectural goal, not a demonstrated reduction in latency or resource use. Capturing and encoding on the game host still consume resources.

> **Project status: experimental and incomplete.** This repository is for development and controlled verification, **not a ready-to-deploy streaming service**. Generated-media tests exercise part of the real production pipeline, but the complete capture-to-production path, physical two-host operation and gameplay non-regression remain open. Returned speech has passed isolated two-Agent software acceptance. The Nix service remains disabled by default.

## How it fits together

```text
Capture host (game stays here)               Production host

Game display + selected game audio
  -> Game capture / encode (GSR) ---------> MediaMTX -> OBS -> recording / stream
                                     RTMP     |          composition / output
Selected microphone
  -> FFmpeg / Opus -----------------------> RTSP -> RNNoise -> private audio -> OBS

Rust capture agent <- authenticated control / SSH -> Rust production agent

Local game playback: independent of production
Production PCM16 WAV -> owned SSH control forward -> selected capture PipeWire sink
```

The capture and production hosts are **roles**, not specific machines. The diagram describes the intended system and existing worker boundaries, **not a fully accepted live flow**. In particular, current generated-media acceptance starts on the production side; it does not exercise GSR on the capture host or the SSH media path. See the [PoC mission and evidence boundaries](docs/memory/poc-mission.md) and [media acceptance profile](docs/media-acceptance.md).

SplitCast is an **orchestrator, not another media engine**. It uses GPU Screen Recorder (GSR) for game capture, FFmpeg for media processing, MediaMTX for transport/relay, and OBS Studio for scene composition and output. Unlike Sunshine, it is not a remote-play host: game input and local playback are meant to stay on the capture machine. Unlike Owncast, it is not an audience-facing livestream server. It does not replace OBS Studio or MediaMTX.

## What works, and what is still missing

| Area | Current state |
| --- | --- |
| Control and configuration | Native CLI, line-mode TUI, Rust/WASM browser client code, typed commands, TOML-first validation, owner-scoped lifecycle and bounded worker cleanup are implemented and covered by local software tests. A built browser client is **not** browser-usage acceptance. |
| Production-side media | A device-free test runs the real production agent with MediaMTX, RNNoise/private audio, and OBS, then checks the resulting recording with an independent decoder. This is **generated-input, production-only** evidence. |
| Capture-to-production transport | Capture-side GSR/FFmpeg workers and SSH forwards exist, but actual two-agent media publication, network impairment, decoded-frame freshness enforcement, and physical capture remain unaccepted. Advancing byte counters alone do not prove fresh video. |
| Returned audio and chat events | The prepared Agents return bounded speech to an explicitly selected capture sink. Two-Agent tests observe the exact player input and independently monitor private PipeWire sinks. EventSub and remote TTS mixing remain gated. [Protocol and software evidence](docs/evidence/audio-return.md). |
| Deployment and recovery | No automatic service start or approved production cutover. Remote cleanup obligations and failure handling exist, but full crash recovery, end-to-end performance, and listener/host acceptance remain open. |

The distinction matters: **implemented code ≠ passed isolated test ≠ accepted deployment**. Read the [production-readiness gates](docs/production-readiness.md), [verification record](docs/verification.md), and [open risks](docs/memory/open-risks.md) before interpreting a green check as a feature guarantee.

## Start safely: build and inspect offline

The supported development target is **Linux `x86_64`**, using the repository's Nix-pinned Rust toolchain. These commands require Nix with flakes enabled and run from the repository root. They do not enable a service, access physical capture devices, start a public stream, or switch a NixOS generation.

```sh
nix build .#default
./result/bin/splitcast selfcheck
./result/bin/splitcast validate --config packages/splitcast-rs/example.toml
./result/bin/splitcast doctor --offline --config packages/splitcast-rs/example.toml
```

`selfcheck` exercises the local lifecycle without starting media. `validate` checks the example configuration offline. The example deliberately selects **no capture target, game-audio application, or microphone**; offline `doctor` should therefore report **not ready** for streaming. That is the expected result, not a failed setup.

For the repository's regular source checks:

```sh
nix develop -c cargo run -p splitcast-xtask -- verify
```

The verifier runs the declared source/test matrix against local software fixtures and stores logs under ignored `artifacts/verification/`. Separate opt-in profiles exercise virtual transport, private audio, and real production software with generated media. **Do not treat them as permission for real SSH hosts, capture hardware, audio changes, public listeners, or broadcasting.** Follow the [virtual acceptance guide](docs/virtual-acceptance.md), [media profile](docs/media-acceptance.md), and [operations runbook](docs/operations.md) for exact scopes and safeguards.

## Packages and integration

The flake exports `packages.x86_64-linux.default`, `packages.x86_64-linux.splitcast-web`, `homeManagerModules.default`, and `nixosModules.default`. A consumer can import these without copying SplitCast sources into its own repository. **Runtime selection is migration-gated** (`rustCandidate = false` by default) and the Home Manager unit has no autostart target; neither building nor importing a module activates streaming.

See [Nix integration](docs/nix-integration.md). A signed GitHub Releases Nix cache is documented for the specifically verified Linux `x86_64` candidate, with pinned source and no-build consumption steps in [binary-cache instructions](docs/binary-cache.md). Cache availability is **distribution evidence, not runtime acceptance**.

## Repository guide

- **Implementation:** [`packages/splitcast-rs`](packages/splitcast-rs) owns native effects; [`packages/splitcast-protocol`](packages/splitcast-protocol) defines shared commands; [`packages/splitcast-web`](packages/splitcast-web) provides the Rust/WASM client; [`tools/xtask`](tools/xtask) runs development checks. The crates are experimental and not published to a registry.
- **Design and evidence:** [Rust design review](docs/design-review.md), [current verification](docs/verification.md), [PoC mission](docs/memory/poc-mission.md), [decisions and unknowns](MEMORY.md), and [import history](docs/history/README.md).
- **Operating boundaries:** [Operations and rollback](docs/operations.md), [security](SECURITY.md), [production-readiness gates](docs/production-readiness.md), and [Nix integration](docs/nix-integration.md).
- **Contributing and licensing:** [Contribution policy](CONTRIBUTING.md), [MIT license for original SplitCast code](LICENSE), and [third-party notices](THIRD_PARTY.md). Dependencies, upstream OBS modifications, and other third-party material retain their own licenses.

New development should target a **specific missing observable behavior**, not expand the orchestration framework ahead of the PoC. The [environment-gap entry](docs/memory/environment-gap.md) identifies the current investigation boundary without authorizing host operations.