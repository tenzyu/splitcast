# Experimental Nix cache, not deployment acceptance

This prerelease contains a Nix binary-cache index and signed project NARs. It is
unverified until the workflow's separate fresh-consumer job succeeds. A later
failure does not become a successful delivery merely because these assets exist.

The tag names the tested source commit, workflow run, and attempt. `manifest.json`
records source and lock identities, exact native and WASM paths, closure
metadata, custom paths, and asset hashes. The public signing key is committed in
`nix/cache-public-key` at that source revision. Verify that key independently
before trusting the cache. Never disable signature checks.

Official dependencies retain their official signatures and download from
`cache.nixos.org`. This release is not an independent mirror of those NARs.
Custom project binaries and patched OBS have accompanying source archives,
vendored Cargo sources and notices, pinned Nixpkgs build expressions, patches,
and `THIRD_PARTY.md`. Original licenses apply, including GPL-2.0-or-later and
FDK-AAC where declared by the selected OBS package. No VOICEVOX binary is included.

SplitCast remains experimental. Runtime gates and autostart remain disabled.
Do not infer permission to capture devices, activate services, change an
operating-system generation, open public listeners, or publish streams.
