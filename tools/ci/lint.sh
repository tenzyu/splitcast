#!/usr/bin/env bash
set -euo pipefail

actionlint
shellcheck tools/ci/*.sh
pyflakes tools/ci/*.py
SPLITCAST_CACHE_PROTOCOL_TEST=1 python3 tools/ci/test_release_cache.py
