#!/usr/bin/env bash
set -euo pipefail

repository=${1:?usage: bash tools/ci/create-cache-key.sh OWNER/REPO}
secret=NIX_CACHE_SIGNING_KEY
existing=$(gh secret list --repo "$repository" --json name --jq ".[] | select(.name == \"$secret\") | .name")
if [[ -n "$existing" ]]; then
  echo "$secret already exists; refusing an implicit key rotation" >&2
  exit 1
fi
umask 077
directory=$(mktemp -d "${TMPDIR:-/tmp}/splitcast-cache-key.XXXXXXXX")
trap 'rm -f "$directory/private" "$directory/public"; rmdir "$directory"' EXIT
nix-store --generate-binary-cache-key splitcast-1 "$directory/private" "$directory/public"
gh secret set "$secret" --repo "$repository" < "$directory/private"
cat "$directory/public"
