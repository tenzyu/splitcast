# Convergence workflow

Use this optional checklist runner for bounded work in a dedicated Git worktree.
It wraps the existing checks. The [verification record](verification.md) and
GitHub carry publishable evidence. Review dispositions belong on the PR, with
nonblocking follow-ups in an existing issue or evidence index when useful.

## Pin a work unit

Start in the intended worktree with the approved direnv shell. When it is
already active, run the commands directly. Otherwise use
`nix develop -c python3 tools/ci/convergence.py …` for the Python entry point.
Use the established `cargo run -p splitcast-xtask -- verify` for Rust checks.

Write a JSON contract in an owner-only directory outside the source worktree.
Set `base` to the full Git ID recorded for this work unit. This example shows
the main commit used to integrate the harness. Set the scope and commands to
the actual task, and review them before enrollment.

```json
{
  "version": 1,
  "id": "review-delivery-docs",
  "base": "8c54a4ecaecaa4e6c292b59886ff6405caf50d15",
  "objective": "Validate delivery documentation for the approved change",
  "scope": ["docs/verification.md"],
  "invariants": ["Keep historical failures and exact source IDs"],
  "checks": [
    {"id": "links", "argv": ["python3", "tools/ci/check_docs.py"], "timeout_seconds": 30}
  ],
  "manual_gates": {
    "functional-poc": "Source-bound two-host observations",
    "game-performance": "Matched OFF/ON/OFF measurements",
    "operational-recovery": "Owner-specific Stop and rollback evidence"
  },
  "forbidden_effects": ["publish stream", "activate services", "switch generation", "merge"],
  "exit_condition": "Links pass and the scoped change is ready for operator review"
}
```

```sh
sha256sum "$contract"
python3 tools/ci/convergence.py init "$contract" --accept "$reviewed_sha256"
python3 tools/ci/convergence.py check "$contract"
python3 tools/ci/convergence.py run "$contract" links
python3 tools/ci/convergence.py status "$contract"
python3 tools/ci/convergence.py handoff "$contract"
```

The digest is an explicit acknowledgment of the reviewed bytes. A changed
contract requires re-evaluation and a new ID. Keep the original contract and
receipts. A linked worktree enrolls separately. Paths are exact filenames or
`directory/**`. Scope checks every layer from base to HEAD, index and actual
working files. Both sides of renames count. Git-ignored outputs are outside
the work receipt. Symlinks, submodules, unmerged indexes and files larger
than 8 MiB block the snapshot. The aggregate read limit is 128 MiB.

`check` inspects scope. `status` gives JSON. `report` and `handoff` render
the same current status as Markdown. Exit `0` means the requested check
passed, is already current, or the command completed. Exit `2` means
`BLOCKED`, a failed check, or invalid input. Exit `3` means software
evidence is missing or stale. Physical and approval gates are independent of
the software result. An open physical blocker appears on its own gate and
blocks the work report when it is explicitly part of the accepted work unit.

Use stable invariant and behavior names across review revisions. Add a local
pointer to a GitHub review, issue, or exact observation with `finding`:

```sh
python3 tools/ci/convergence.py finding "$contract" \
  --axis code --classification BLOCKER --invariant preserve-evidence \
  --behavior missing-run-source --owner maintainer --evidence "$review_url"
```

The invariant, affected behavior and acceptance axis identify one finding.
Updates retain the evidence pointer and current code identity. Use
`FOLLOW_UP` or `QUESTION` for nonblocking items. Resolve an open blocker
with `--disposition RESOLVED` and new evidence. Refer to the original
review thread for its history and disposition. Separate operator decisions
cover merge and any eventual deployment.

## Receipts and recovery

Only run reviewed commands and source. They execute locally without a shell
launcher or a new sandbox. A worktree separates checkout state. Avoid secret
values in contracts, argv, test inputs and output. The runner forwards a
fixed build-environment allowlist and fingerprints its values with a
private key. Cargo, Nix and other programs can still read their normal local
configuration. Pin external inputs through the existing tooling. The receipt
covers the supplied argv, executable file identity, environment scope and the
source and index observed at each endpoint. Concurrent transient edits,
ignored inputs, daemon state and external dependencies require their own
controls.

The report names the receipt and log identity. Find them under the owner-only
`convergence/<id>/` directory inside `git rev-parse --absolute-git-dir`.
The log is a bounded sanitized record of stdout/stderr. It redacts common
secret markers and known secret environment values. Review any excerpt
before sharing. A successful receipt is reused only while its inputs match
and its log still hashes to the recorded identity. Stop when the exit
condition is met.

After failure, inspect the actual retained output and correct the cause.

```sh
python3 tools/ci/convergence.py inspect "$contract" attempt-000001 --tail 10
python3 tools/ci/convergence.py inspect "$contract" attempt-000001 \
  --classify INFRASTRUCTURE --line "$observed_line_number"
```

Classification is an explicit assessment tied to a numbered line in the
sanitized log. Use `CODE` or `UNKNOWN` when the evidence calls for it.
Three failed attempts at identical inputs end retries. Review the receipt
before retrying after a material change. For a timeout, verify the recorded
process-group kill and leader reap. An escaped child or kernel-stuck process
can outlive the deadline. If a runner is found interrupted or cleanup is
uncertain, inspect the owned process and receipt before any new check.
Keep the failed or incomplete attempt in the record.

## Wiki maintenance

Use the existing [Dreaming contract](memory/dreaming.md) and
[update protocol](memory/update-protocol.md). Run the scan before opening an
agent session. Keep historical targets and host evidence as recorded.

```sh
python3 tools/ci/convergence.py wiki scan --summary
python3 tools/ci/convergence.py wiki dream
python3 tools/ci/convergence.py wiki dream --include-unbaselined --limit 1
# Opt in from a clean, committed checkout to request a local proposal.
python3 tools/ci/convergence.py wiki dream --execute --limit 1
```

The scan is local and deterministic. `STALE_CANDIDATE` means a declared
dependency changed. `UNAFFECTED` describes dependency comparison, paired
with `UNBASELINED` or `UNVERIFIED` when applicable. `HISTORICAL` pages remain
read-only for the gardener. Missing paths and broken links block the run.
Use a full commit ID with `wiki scan --since` for a one-off comparison.

Dreaming uses the installed Codex CLI and its authenticated provider when
execution is requested. The runner gives it a disposable detached checkout
with separate Git metadata and validates the resulting diff. Codex gets a
workspace-write request with shell network disabled. Trust the installed CLI
and reviewed checkout. The provider receives the targeted prompt and source
read during the run. The harness records counts and disposition, while the
candidate patch stays owner-only under `convergence-wiki/` in the Git common
directory. Review the patch and its source before applying it manually on a
review branch. An empty or already handled queue launches no model.
Candidates are keyed by the page and its dependency contents. Shared Git
metadata serializes proposal generation across linked worktrees. A failed
batch has a three-attempt ceiling on the same page inputs.

CI runs the Python tests, source-pinned Wiki scan, existing link and OKF
checks, and the existing Rust gates. CI does not execute Codex. Replay the
recorded checks for any changed source identity. Roll back the integration
with `git revert <integration-commit>` after review. Local receipts and
proposal patches can be retained for audit.
