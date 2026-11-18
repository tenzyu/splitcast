#!/usr/bin/env bash
set -euo pipefail

actionlint
shellcheck tools/ci/*.sh
pyflakes tools/ci/*.py
if [[ -n "${GITHUB_SHA:-}" ]]; then
    test "$(git rev-parse HEAD)" = "$GITHUB_SHA"
fi
python3 -m unittest discover -s tools/ci -p 'test_convergence*.py'
if [[ -n "${GITHUB_SHA:-}" ]]; then
    python3 tools/ci/convergence.py wiki scan --ci-sha "$GITHUB_SHA" --summary
else
    python3 tools/ci/check_docs.py
    python3 tools/ci/convergence.py wiki scan --summary
fi
SPLITCAST_CACHE_PROTOCOL_TEST=1 python3 tools/ci/test_release_cache.py
python3 tools/ci/test_finalize_release.py
