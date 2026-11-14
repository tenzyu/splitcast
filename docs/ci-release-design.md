# Signed GitHub release cache

## Usage

The maintainer dispatches `release-cache.yml` on `main` and explicitly confirms
publication. The workflow validates that exact commit, prepares a cache on a
fresh unprivileged runner, and promotes only that run's bundle. Another fresh
runner tests the public release without building either output.

Consumers use the selected release's cache URL, the committed public key in
`nix/cache-public-key`, and the same source commit and locks. The source-built
flake stays authoritative. A consumer that changes inputs is not covered.

## Existing ownership

The packaging explainer confirmed that `default`, `splitcast`, and
`splitcast-rs-core` are aliases for one wrapped package, not a raw executable.
Its wrapper retains web assets, RNNoise and multimedia tools, including the
loopback-patched OBS. `packages.sh` owns release builds and offline executable
checks. `nix/source.nix` owns the source fileset. A raw executable download would
drop these dependencies and cannot replace the current package.

## Synthesis decision

Three independent sketches were read in full. All chose a complete narinfo
index that forwards official NAR downloads and hosts only custom NARs. Full
closure mirroring would redistribute far more third-party binaries. A raw
binary-only flake would change package identity and leave runtime builds.

Sol's sketch is the base. Terra's cross-judgment preferred its trusted same-run
unprivileged build bundle over rebuilding after introducing publication
credentials. The final design also adopts Terra's explicit redistribution
inventory and Luna's versioned manifest. No PR artifact, store cache or other
workflow run can supply publisher inputs. A rerun uses a new run-attempt tag;
published tags and assets are never overwritten.

## Interfaces and invariants

`tools/ci/release_cache.py` owns one manifest and three operations:

```text
prepare(evidence, sources, destination) -> unsigned cache and manifest
sign(destination, expected_manifest_hash) -> signed cache and final manifest
consume(cache_url, expected_manifest_hash) -> retained no-build evidence
```

The manifest binds source SHA, both locks, system, named roots, complete closure,
custom entries and asset digests. Each closure member has exactly one narinfo.
Official metadata must match local store identity, NAR hash, size and references.
Only its URL changes, to an absolute official-cache URL. Custom NAR URLs become
flat release-asset names. Nix verifies signatures, not a handwritten verifier.

`release-sources.sh` provides the exact project/build sources, vendored Cargo
sources with notices, upstream OBS source and pinned Nixpkgs build expressions.
Only the expected project native/web and patched-OBS paths may be custom. A new
custom dependency fails classification and needs another redistribution review.

The signing key exists only in the signing step's environment and a temporary
0600 file outside the store and artifact tree. The publisher has release-write
permission only after all build checks succeed. It verifies the same-run
manifest hash, signs custom entries, verifies the entire index, creates a unique
draft, uploads and checks assets, then publishes an unverified prerelease. A
successful cold consumer establishes delivery evidence separately.

## Failure and verification contract

Reject wrong source, locks, roots, missing references, mismatched upstream data,
foreign URLs, unknown custom outputs, missing source materials, bad signatures,
unsafe asset names and release limits. An upstream error is not a cache miss.
Failed uploads stay draft. Failed consumption leaves an unverified prerelease.
No failure permits an unsigned download, fallback build, tag rewrite, or disabled
test assertion.

Tests cover validation and signature failures with generated fixture keys.
Hosted consumption requires absent roots before substitution, exact output
identity afterward, signature enforcement, and packaged selfcheck and validate.
The release is the only substituter, though forwarded NAR bytes still come from
the official cache. Record that dependency rather than claiming a full mirror.

No design or passing test enables runtime, autostart, capture or deployment.
Rollback removes only CI/cache code or consumer cache configuration. Key
revocation is separate from source rollback.
