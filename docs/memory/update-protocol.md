---
type: Memory Maintenance Protocol
title: update-protocol
description: Evidence-preserving update and correction rules.
status: stable
sources:
  - { id: primary, resource: dreaming.md }
---
# Update SplitCast knowledge

Use this procedure after a relevant session, design change, failed hypothesis, or verification run. Keep unrelated transcripts and machine histories out of scope.

1. Read `MEMORY.md`, the relevant decision, and its sources before repeating an investigation.
2. Identify the repository, exact commit or dirty-tree description, artifact, date with timezone when known, role, and environment. Separate source checkout, built artifact, and running generation.
3. Assign a new source ID in [the register](evidence.md). Record a durable repository-relative path, Git object, or artifact identity. Mark operator reports and unavailable raw material explicitly.
4. Record the goal, adopted and rejected choices, rationale, and the observation that changed the hypothesis. State what remains unknown.
5. For each check, record the exact command, outcome, and observable behavior. Distinguish compiler/environment failures, fixture failures, behavioral failures, opt-in skips, and unrun checks.
6. Append a dated correction that names the superseded claim and its source IDs. Preserve the earlier decision and link both directions. Do not silently rewrite a failed hypothesis as if it had never existed.
7. Update the affected risk and index summary. Close a risk only when evidence covers the stated contract. A narrower passing result leaves the rest open.
8. Inspect the knowledge diff for secrets, personal paths, machine aliases, broken references, and accidental promotion of historical or synthetic results into standalone acceptance.

## Correction record

Use the following fields in the affected page:

```text
Correction ID and recorded date:
Earlier claim and source IDs:
New observation and artifact identity:
Supersedes or narrows:
Decision changed and why:
Still unknown:
Verification command and result, or NOT RUN:
Related decisions, evidence, and risks:
```

Use `capture host` and `production host` in published knowledge. Keep sanitized publication copies separate from raw originals. If a filename changes, retain the source ID and original Git blob so citations remain traceable.

Split or merge pages when lookup becomes harder. Preserve IDs and crosslinks through moves. Never delete raw evidence, overwrite user work, or infer authorization from an old session's approval.

## Current memory verification boundary

The 2026-11-13 memory pass read scoped historical documents and inspected original Git identities. It did not run application tests, contact hosts, activate services, or verify the standalone extraction. Register subsequent fresh results separately from E01–E05.

## Progressive retrieval and Dreaming (2026-11-17)

The [Dreaming map](dreaming.md) is the current consolidation protocol, informed
by Anthropic's 2026-11-06 [agent-memory research](https://www.anthropic.com/research/components-of-agent-memory).
Use the hierarchy `MEMORY.md → relevant domain page → exact source/receipt`.
Do not preload every `.md` document, and do not treat a compressed session
summary or a merged PR as runtime evidence. A new run, contradiction or merge
should update the minimum current index and risk pointers while leaving the
original evidence ledger and historical snapshots intact.

When a live source contradicts a stale current paragraph, document the dated
supersession on the affected concept page. Change the cold-start capsule only when a durable constraint or its retrieval route changes. Never rewrite the historical
observation to imply it had always been correct. For uncertain CI fixes, link
the specific failing run and candidate PR; upgrade to PASS only after the exact
amended commit is verified.

For automated change detection, use the
[incremental Dreaming workflow](dreaming.md#incremental-proposals).
Treat a saved proposal as a review input. Add a review baseline only after
reconciling the exact page and declared sources with the durable evidence.

## When a proposal moves

The [proposal snapshot](proposed-revisions.md) records what was knowable on its date. The local provenance scan compares declared files. A green scan cannot see a PR retarget, closure, force-push or merge on GitHub. On a relevant change, inspect the affected PR and its stack neighbor:

```sh
gh pr view "$pr" --json state,isDraft,baseRefName,headRefOid,mergeCommit,mergedAt
git ls-remote origin refs/heads/main 'refs/pull/18/head' 'refs/pull/19/head' 'refs/pull/20/head'
```

Read the merge commit on main and compare the actual functions/tests with the pinned proposal, including squash merges. A new head invalidates the old head as a description of the live proposal. A change to #19's base, head or merge state also requires checking #20's base and its contract dependency. Preserve the earlier CI run, tested tree and failed observations. Amend the relevant row and source ID, then adjust the affected domain page and its exact local dependencies in [provenance](provenance.json). Once files land, replace proposal-only lookup routes with main paths. Update physical state only with a matching host receipt. Use the [incremental scan and optional proposal](../convergence.md#wiki-maintenance) on the new tree.
