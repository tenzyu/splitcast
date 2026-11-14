#!/usr/bin/env bash
set -euo pipefail

directory=${1:?usage: bash tools/ci/publish-release.sh SIGNED_DIRECTORY}
[[ "$GITHUB_EVENT_NAME" == workflow_dispatch && "$GITHUB_REF" == refs/heads/main ]]
[[ "$(git rev-parse HEAD)" == "$GITHUB_SHA" ]]
tag="nix-cache-${GITHUB_SHA}-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}"
gh api --method POST "repos/$GITHUB_REPOSITORY/git/refs" -f ref="refs/tags/$tag" -f sha="$GITHUB_SHA" > artifacts/ci/release-tag.json
gh api --method POST "repos/$GITHUB_REPOSITORY/releases" \
  -f tag_name="$tag" -f target_commitish="$GITHUB_SHA" \
  -f name="Unverified experimental Nix cache ${GITHUB_SHA:0:12}" \
  -F draft=true -F prerelease=true -f make_latest=false \
  -F body=@docs/release-cache-notes.md > artifacts/ci/release-draft.json
release_id=$(python3 -c 'import json; print(json.load(open("artifacts/ci/release-draft.json"))["id"])')
[[ "$release_id" =~ ^[0-9]+$ ]]
gh release upload "$tag" "$directory"/* --repo "$GITHUB_REPOSITORY"
gh api --paginate --slurp "repos/$GITHUB_REPOSITORY/releases/$release_id/assets?per_page=100" > artifacts/ci/release-assets.json
python3 - "$directory" <<'PY'
import json
from pathlib import Path
import sys
from tools.ci.release_cache import verify_uploaded_assets

verify_uploaded_assets(Path(sys.argv[1]), json.loads(Path("artifacts/ci/release-assets.json").read_text()))
PY
gh api --method PATCH "repos/$GITHUB_REPOSITORY/releases/$release_id" \
  -F draft=false -F prerelease=true -f make_latest=false > artifacts/ci/release-published.json
printf 'cache_url=https://github.com/%s/releases/download/%s\n' "$GITHUB_REPOSITORY" "$tag" >> "$GITHUB_OUTPUT"
