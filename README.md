# SplitCast

SplitCast is an experimental Rust controller for capture on one host and production on another. It owns native media workers instead of injecting code into a game. Local playback and input must remain independent of the production host.

**This is a partial implementation, not a production-ready streaming system.** The Nix module keeps the runtime migration-gated by default and defines no autostart target. Speech delivery, EventSub, shared selection transactions, full recovery, live media acceptance, and gameplay non-regression remain open.

## Build and verify

The supported development environment is Linux x86-64 with Rust 1.95.0, pinned through Nix. This is a conservative supported floor, not a measured historical minimum compiler version.

```sh
nix develop -c cargo run -p splitcast-xtask -- verify
nix build .#default .#splitcast-web
result/bin/splitcast validate --config packages/splitcast-rs/example.toml
result/bin/splitcast doctor --offline --config packages/splitcast-rs/example.toml
```

The example has no selected capture or microphone. Doctor must not report it ready to stream. Verification logs go to the ignored `artifacts/verification/` directory. Real audio-daemon and generated-media tests are opt-in; see [operations](docs/operations.md). Ordinary tests use isolated local peers and stand-in workers, not physical capture devices.

## What exists

- A typed command protocol shared by the native CLI, line-mode TUI, and Rust/WASM browser client.
- TOML-first configuration with the same strict native validation for JSON, finite routes, and private credential files.
- Phase-owned local authority, remote preparation obligations, explicit output selection, and bounded process-group teardown.
- Native bounded loopback HTTP and OBS WebSocket control, plus an isolated production audio server.
- A tested synthesis component whose incomplete delivery and playback path is refused by the live agent.

`packages/splitcast-rs` owns native effects. `packages/splitcast-protocol` is the shared command boundary. `packages/splitcast-web` contains browser behavior; its handwritten JavaScript only loads generated WASM glue. `tools/xtask` runs repeatable development checks. Crates are not configured for registry publication; public Rust interfaces remain experimental.

## Use from another flake

The repository exports `packages.x86_64-linux.default`, `homeManagerModules.default`, and `nixosModules.default`. A consumer imports the modules and supplies deployment settings. It does not copy this repository's source into its own tree. See [Nix integration](docs/nix-integration.md).

## Review and evidence

- [Rust design review](docs/design-review.md) covers contracts, ownership, failure, cancellation, public API, performance, and risks.
- [Verification](docs/verification.md) distinguishes fresh standalone results from historical source results.
- [Operations](docs/operations.md) gives test steps, stop conditions, acceptance gates, and rollback.
- [Memory](MEMORY.md) connects goals, decisions, disproved assumptions, source evidence, and unknowns across sessions.
- [History](docs/history/README.md) describes normalized source records and original Git blob identities.

Read [contribution policy](CONTRIBUTING.md), [security boundaries](SECURITY.md), and [third-party notices](THIRD_PARTY.md) before extending or publishing this candidate. Original SplitCast code uses the [MIT license](LICENSE); third-party material retains its applicable license.
