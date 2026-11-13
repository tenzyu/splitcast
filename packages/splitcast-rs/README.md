# Native SplitCast candidate

This crate owns native control, configuration, media workers, and lifecycle effects. It is part of the root Cargo workspace and uses the root lockfile.

```sh
nix develop -c cargo test -p splitcast-rs-core --locked --offline
nix build .#default
result/bin/splitcast validate --config packages/splitcast-rs/example.toml
```

The binary remains `splitcast-rs-core`; the Nix wrapper also exposes `splitcast`. Configuration and CLI formats are experimental downstream commitments. The default example is deliberately unselected. No package build authorizes capture, external listeners, deployment, or public publication.

Read the [root README](../../README.md), [design review](../../docs/design-review.md), and [operations runbook](../../docs/operations.md). The historical Python implementation is not installed or delegated to. Source history is a development reference, not a fallback runtime.
