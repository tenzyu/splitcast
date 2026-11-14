#!/usr/bin/env bash
set -euo pipefail

# Local loopback and owned stand-in workers only. Each test asserts the blocked
# operation boundary, authorization negatives, acknowledgment, and cleanup.
root=$(git rev-parse --show-toplevel)
cd "$root"
logs="$root/artifacts/verification/stop-admission-$(date +%s%N)"
mkdir -p "$logs"
git rev-parse HEAD > "$logs/revision.txt"
git diff > "$logs/source.patch"
git status --short > "$logs/working-tree.txt"
git ls-files -z Cargo.toml Cargo.lock rust-toolchain.toml flake.nix flake.lock packages tools \
    | xargs -0 sha256sum > "$logs/source-sha256.txt"
failed=0
for scenario in prepare failure-cleanup; do
    if [[ "$scenario" == prepare ]]; then
        test_case=agent::lifecycle_tests::shutdown_during_delayed_prepare_closes_connections_and_stops_receiver
        acknowledgment='delayed Prepare Stop intent admission:'
        intent="$acknowledgment"
        cleanup='delayed Prepare Stop cleanup observed'
    else
        test_case=agent::lifecycle_tests::stop_acknowledges_during_failure_cleanup_before_remote_ack_is_released
        acknowledgment='failure cleanup Stop acknowledgment:'
        intent='failure cleanup Stop intent observed:'
        cleanup='failure cleanup Stop cleanup observed'
    fi
    for run in 1 2 3 4 5; do
        log="$logs/$scenario-$run.log"
        if nix develop -c cargo test -p splitcast-rs-core --lib "$test_case" \
            --locked --offline -- --exact --nocapture > "$log" 2>&1; then
            if grep -q '^test result: ok. 1 passed;' "$log" \
                && grep -q "$acknowledgment" "$log" \
                && grep -q "$intent" "$log" \
                && grep -q "$cleanup" "$log"; then
                printf '%s\t%s\tPASS\t0\n' "$scenario" "$run" >> "$logs/summary.tsv"
            else
                printf '%s\t%s\tFAIL\tmissing-observation\n' "$scenario" "$run" >> "$logs/summary.tsv"
                failed=1
            fi
        else
            code=$?
            printf '%s\t%s\tFAIL\t%s\n' "$scenario" "$run" "$code" >> "$logs/summary.tsv"
            failed=1
        fi
    done
done
cat "$logs/summary.tsv"
grep -h -e 'delayed Prepare Stop' -e 'failure cleanup Stop' "$logs"/*.log || true
printf 'evidence: %s\n' "$logs"
exit "$failed"
