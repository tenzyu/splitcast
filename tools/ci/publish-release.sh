#!/usr/bin/env bash
set -euo pipefail

directory=${1:?usage: bash tools/ci/publish-release.sh SIGNED_DIRECTORY}
[[ "$GITHUB_EVENT_NAME" == workflow_dispatch && "$GITHUB_REF" == refs/heads/main ]]
[[ "$(git rev-parse HEAD)" == "$GITHUB_SHA" ]]
tag="nix-cache-${GITHUB_SHA}-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}"
gh api --method POST "repos/$GITHUB_REPOSITORY/git/refs" -f ref="refs/tags/$tag" -f sha="$GITHUB_SHA" > artifacts/ci/release-tag.json
gh release create "$tag" --repo "$GITHUB_REPOSITORY" --verify-tag --draft --prerelease --latest=false \
  --title "Unverified experimental Nix cache ${GITHUB_SHA:0:12}" --notes-file docs/release-cache-notes.md
gh release upload "$tag" "$directory"/* --repo "$GITHUB_REPOSITORY"
release_id=$(gh api "repos/$GITHUB_REPOSITORY/releases/tags/$tag" --jq .id)
gh api --paginate --slurp "repos/$GITHUB_REPOSITORY/releases/$release_id/assets?per_page=100" > artifacts/ci/release-assets.json
python3 - "$directory" <<'PY'
import json
from pathlib import Path
import sys
from tools.ci.release_cache import digest

directory = Path(sys.argv[1])
manifest = json.loads((directory / "manifest.json").read_text())
expected = manifest["assets"] | {"manifest.json": {"sha256": digest(directory / "manifest.json"), "size": (directory / "manifest.json").stat().st_size}}
assets = [asset for page in json.loads(Path("artifacts/ci/release-assets.json").read_text()) for asset in page]
observed = {asset["name"]: {"sha256": asset["digest"].removeprefix("sha256:"), "size": asset["size"]} for asset in assets}
if len(assets) != len(observed) or observed != expected:
    raise SystemExit("Remote release assets differ. Leave this release draft.")
PY
gh release edit "$tag" --repo "$GITHUB_REPOSITORY" --draft=false --prerelease --latest=false
printf 'cache_url=https://github.com/%s/releases/download/%s\n' "$GITHUB_REPOSITORY" "$tag" >> "$GITHUB_OUTPUT"
