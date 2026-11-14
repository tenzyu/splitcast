# Release-cache design input

The user approved signed GitHub Releases as the Nix cache, including generating
a signing key, storing it in a GitHub secret, and publishing a verification
prerelease after checking redistribution conditions. No deployment is approved.
The example was `hiraginoyuki/onair`'s release workflow and binary-only flake.
Do not copy its force-pushed tags or automatic main rewrites.

Existing shape:

- `flake.nix` exports only x86_64-linux. `.#default` wraps the native binary
  with WASM assets, RNNoise, patched OBS, FFmpeg, GSR, MediaMTX and other tools.
- Source checks and packages must retain their existing semantics. Do not
  replace patched OBS or remove dependencies to make distribution easier.
- `tools/ci/packages.sh` builds `.#default` and `.#splitcast-web`, records the
  derivations and closure, runs `nix flake check`, selfcheck and example validate.
- CI workflow has ordinary, virtual and package jobs on fresh hosted runners,
  read-only permissions, pinned actions and allowlisted evidence, no caches.
- Public repo `tenzyu/splitcast`, default main. Work is on `ci/nix-cache`, draft
  PR #1. The previous implementation was preserved at `7585308`.
- Local new package build passed. Closure JSON has 442 paths, so root-level
  metadata plus a few custom NARs fits GitHub's 1,000-release-asset limit.
- `THIRD_PARTY.md` calls out GPL modified OBS, codec licensing and source
  obligations. Imported documentation publication is a separate historic gate.

Consumer contract: exact same Git commit, locks and output paths as validated;
fresh ephemeral runner, no local or remote builds, authenticated NARs, direct
query of the intended cache, selfcheck and example validate without any agent.
Source consumer's `follows` can change paths and is not covered by this claim.

Possible static-cache transport: GitHub release assets have a flat namespace.
Nix's `nix-cache-info`, `<hash>.narinfo` and compressed NAR files can all be
assets. Change local narinfo URL from `nar/<file>` to `<file>`. Signatures cover
store identity/hash/references, not URL. Official cache metadata can retain its
upstream signature and use an absolute `https://cache.nixos.org/nar/...` URL,
avoiding redistribution of the entire multimedia closure. Missing upstream
paths need signed custom NARs and corresponding source/license materials.
This is a proposed mechanism, not yet proven in this repository.

Compare structurally distinct candidates before synthesis. Provide usage first,
module map, signatures/pseudocode and rationale, not implementation. Keep changes
small. Model failed uploads and reruns, malicious PRs, SHA mismatches, tampering,
asset-count/size limits and upstream cache outages explicitly. Do not generate
keys, upload anything or change repository files during design.
