# Obtain the verified prebuilt packages

The experimental `x86_64-linux` delivery at commit
`d2205d06b0dc695c3d9b7372fafa7facee769fa9` passed
[the independent no-build consumer](https://github.com/tenzyu/splitcast/actions/runs/43561723796).
It includes native SplitCast, WASM assets and the selected patched OBS.
Official dependency NARs remain on `cache.nixos.org`; GitHub Releases hosts
the complete cache index and three project-specific NARs. No Cachix account
or cache credential is needed to download them.

## Download without compilation

Use Nix on Linux x86-64. For multi-user Nix, the administrator must authorize
the cache URL and public keys, or the command must run as a Nix-trusted user.
Do not disable signature checks or change a host service to bypass that policy.
Verify the public key against `nix/cache-public-key` at the pinned source commit.

```sh
source_ref='github:tenzyu/splitcast/d2205d06b0dc695c3d9b7372fafa7facee769fa9'
cache='https://github.com/tenzyu/splitcast/releases/download/nix-cache-d2205d06b0dc695c3d9b7372fafa7facee769fa9-43561723796-1'
keys='splitcast-1:vTbhAaBTWjRfVHQekUUYwSb9GBftwHNgBUhXGk0jGwM= cache.nixos.org-1:6NCHdD59X431o0gWypbMrAURkbJ16ZPMQFGspcDShjY='

nix build "$source_ref#default" "$source_ref#splitcast-web" \
  --no-link --print-out-paths --max-jobs 0 --builders '' \
  --option substituters "$cache" --option trusted-public-keys "$keys" \
  --option require-sigs true
```

The command prints the native and web output paths. If a required object is
unavailable, it fails instead of compiling. These options apply to this command;
they do not configure a persistent host cache or enable SplitCast.

To check the native package without starting an agent or media:

```sh
nix run "$source_ref" --max-jobs 0 --builders '' \
  --option substituters "$cache" --option trusted-public-keys "$keys" \
  --option require-sigs true -- selfcheck
```

For configuration and runtime boundaries, read [the operations runbook](operations.md).
A consumer that changes Nixpkgs through `follows`, changes the source or locks,
or uses another architecture can produce different store paths. The verified
cache hit does not cover those variants. Source builds remain available.

## Publish a subsequent candidate

Review the exact three custom outputs, dependency changes, source archives,
license notices and codec conditions. Then dispatch from the trusted `main`
branch with both explicit confirmations:

```sh
gh workflow run release-cache.yml --ref main \
  -f publish=true -f reviewed_redistribution=true
```

The workflow validates and builds in an unprivileged job, signs only that run's
bundle in a separate job, publishes a unique initially unverified prerelease,
and tests it on a fresh runner with builds prohibited. Inspect all three job
conclusions and retained evidence before recording delivery success. Never
replace an existing release's assets or force-move its tag. Failed drafts and
runs remain separate evidence. Do not rerun the obsolete first failed draft.

The private signing key is already stored as `NIX_CACHE_SIGNING_KEY`. Do not
regenerate it implicitly or paste it into chat. Key revocation or rotation
requires a separate decision and updated consumer trust. Removing this cache
from a consumer does not roll back or activate a deployment.

This is delivery evidence, not Production Ready status. The actual agent media
path and the remaining [readiness gates](production-readiness.md) still need
their own acceptance. [The complete record](https://github.com/tenzyu/splitcast/blob/archive/pre-dreaming-20261117/docs/evidence/ci-nix-cache-20261114.md)
retains the failed attempts and exact tested identities.
