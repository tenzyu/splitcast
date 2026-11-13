# Third-party material

The native Rust crate previously declared MIT licensing. This repository retains that license for original SplitCast code. Cargo dependencies and external media programs retain their own licenses. `cargo metadata` records dependency license declarations; these declarations are not a legal audit.

`nix/obs-websocket-loopback.patch` modifies OBS Studio and its bundled WebSocket server. That patch follows the upstream OBS Studio GPL-2.0-or-later licensing, not the root MIT grant. Nix obtains upstream OBS separately; this repository does not contain a vendored OBS source tree or proprietary game libraries.

`RUST_AGENT_ROOT.md` and `RUST_ENGINEERING_AGENTS.md` are operator-supplied project policies, copied without changing their content. Historical records under `docs/history/` are normalized migration evidence. Their originals remain in the source repository's Git history and are identified by content hashes in `docs/history/import-manifest.json`. External quotations, references and upstream material retain their applicable rights.

Before public release, the repository owner must confirm their right to publish the imported documentation and policies. No crate is configured for registry publication in this initial extraction.
