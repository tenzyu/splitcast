# Third-party material

The native Rust crate previously declared MIT licensing. This repository retains that license for original SplitCast code. Cargo dependencies and external media programs retain their own licenses. `cargo metadata` records dependency license declarations; these declarations are not a legal audit.

`nix/obs-websocket-loopback.patch` modifies OBS Studio and its bundled WebSocket server. That patch and its historical copy under `docs/history/source/modules/` follow upstream GPL-2.0-or-later licensing, not the root MIT grant. Both copies have adjacent `.license` notices. The upstream license text is retained verbatim in `LICENSES/GPL-2.0-or-later.txt`.

The modified upstream `plugins/obs-websocket/src/websocketserver/WebSocketServer.cpp` credits Stephane Lepin for 2016–2021 and Kyle Manning for 2020–2021 and explicitly permits GPL version 2 or later. Those copyright notices are preserved in the patch sidecars. Nix obtains upstream OBS separately; this repository does not contain a vendored OBS source tree or proprietary game libraries. OBS package metadata also declares FDK-AAC for its codec dependency; this repository does not relicense that dependency or redistribute its source as original SplitCast code.

`RUST_AGENT_ROOT.md` and `RUST_ENGINEERING_AGENTS.md` are operator-supplied project policies, copied without changing their content. Historical records under `docs/history/` are normalized migration evidence. Their originals remain in the source repository's Git history and are identified by content hashes in `docs/history/import-manifest.json`. External quotations, references and upstream material retain their applicable rights.

Before public release, the repository owner must confirm their right to publish the imported documentation and policies. No crate is configured for registry publication in this initial extraction.

## Experimental Nix binary distribution

The release-cache workflow hosts only the selected SplitCast native and web
outputs and the modified OBS output when absent from the official cache. A new
custom dependency stops preparation for a separate review. Official dependency
NARs remain hosted by `cache.nixos.org`; release metadata points to those exact
objects and retains their signatures. VOICEVOX is not selected for distribution.

Each cache release accompanies project binaries with `project-source.tar.xz`
and `cargo-sources.tar.xz`. The latter contains the locked Cargo dependency
sources and their original license and notice files. `obs-source.tar.xz`
contains the selected upstream OBS source, including bundled submodules and
their notices. The project source includes the loopback patch, license sidecar,
GPL text, Cargo manifests and locks, Nix packaging, and the pinned flake lock.
`nixpkgs-source.tar.xz` supplies the corresponding upstream Nix build expressions
and patches. Extract the project source and run the declared flake build to
reconstruct the selected packages. No imported historical documents or
operator-policy bundle is added to these source assets.

The selected OBS package declares both GPL-2.0-or-later and FDK-AAC licenses.
Its linked codec libraries retain their own conditions. Nixpkgs declares these
licenses redistributable, but package metadata is not a legal audit or a patent
license. This distribution preserves the upstream build choice, supplies source
and notices, and makes no broader relicensing claim. The repository owner is
responsible for confirming that the intended distribution meets applicable
copyright, codec, and jurisdiction-specific requirements.
