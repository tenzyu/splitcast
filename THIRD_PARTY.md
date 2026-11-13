# Third-party material

The native Rust crate previously declared MIT licensing. This repository retains that license for original SplitCast code. Cargo dependencies and external media programs retain their own licenses. `cargo metadata` records dependency license declarations; these declarations are not a legal audit.

`nix/obs-websocket-loopback.patch` modifies OBS Studio and its bundled WebSocket server. That patch and its historical copy under `docs/history/source/modules/` follow upstream GPL-2.0-or-later licensing, not the root MIT grant. Both copies have adjacent `.license` notices. The upstream license text is retained verbatim in `LICENSES/GPL-2.0-or-later.txt`.

The modified upstream `plugins/obs-websocket/src/websocketserver/WebSocketServer.cpp` credits Stephane Lepin for 2016–2021 and Kyle Manning for 2020–2021 and explicitly permits GPL version 2 or later. Those copyright notices are preserved in the patch sidecars. Nix obtains upstream OBS separately; this repository does not contain a vendored OBS source tree or proprietary game libraries. OBS package metadata also declares FDK-AAC for its codec dependency; this repository does not relicense that dependency or redistribute its source as original SplitCast code.

`RUST_AGENT_ROOT.md` and `RUST_ENGINEERING_AGENTS.md` are operator-supplied project policies, copied without changing their content. Historical records under `docs/history/` are normalized migration evidence. Their originals remain in the source repository's Git history and are identified by content hashes in `docs/history/import-manifest.json`. External quotations, references and upstream material retain their applicable rights.

Before public release, the repository owner must confirm their right to publish the imported documentation and policies. No crate is configured for registry publication in this initial extraction.
