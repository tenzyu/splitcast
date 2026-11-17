#!/usr/bin/env bash
set -euo pipefail

actionlint
shellcheck tools/ci/*.sh
pyflakes tools/ci/*.py
python3 tools/ci/check_docs.py
SPLITCAST_CACHE_PROTOCOL_TEST=1 python3 tools/ci/test_release_cache.py
python3 tools/ci/test_finalize_release.py
