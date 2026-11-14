#!/usr/bin/env bash
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
if [[ "$PWD" != "$root" ]]; then
    echo 'Run tools/verify-media.sh from the repository root.' >&2
    exit 1
fi
export PYTHONDONTWRITEBYTECODE=1
mkdir -p artifacts/media-plane/runs
evidence="$(mktemp -d "$root/artifacts/media-plane/runs/$(date -u +%Y%m%dT%H%M%SZ)-XXXXXXXX")"
export SPLITCAST_MEDIA_EVIDENCE_DIR="$evidence"
python3 tools/media_acceptance.py --source-identity > "$evidence/build-source.json"
python3 -m unittest discover -s tools -p test_media_acceptance.py -v
cargo build -p splitcast-rs-core --locked --offline
nix-store --query --requisites "$(dirname "$(dirname "$(readlink -f "$(command -v obs)")")")" > "$evidence/obs-closure.txt"
python3 tools/media_acceptance.py --build-receipt > "$evidence/build.json"
exec bwrap --unshare-all --die-with-parent --new-session \
    --ro-bind /nix/store /nix/store --dev /dev --proc /proc --tmpfs /tmp \
    --dir /run --dir /etc \
    --ro-bind "$(readlink -f "$(command -v bash)")" /bin/sh \
    --ro-bind /etc/passwd /etc/passwd --ro-bind /etc/group /etc/group \
    --ro-bind "$root" "$root" --bind "$evidence" /evidence --chdir "$root" \
    --setenv SPLITCAST_MEDIA_HOST_EVIDENCE "$evidence" \
    --setenv SPLITCAST_MEDIA_EVIDENCE_DIR /evidence \
    --setenv SPLITCAST_MEDIA_SANDBOX 1 \
    python3 tools/media_acceptance.py "$@"
