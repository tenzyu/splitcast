# Memory index

Start with [MEMORY.md](../../MEMORY.md), then follow the question to its evidence.

- [Decisions](decisions.md) records the goal, adopted and rejected shapes, contrary observations, and unresolved limits.
- [Evidence](evidence.md) separates the source repository, historical repaired artifacts, and fresh standalone verification.
- [Open risks](open-risks.md) lists the evidence required before a claim can be closed.
- [Update protocol](update-protocol.md) preserves superseded judgments and source references.
- [Current working state](session-state.md) is the cold-start capsule, including uncommitted source, corrected review evidence, and next-session limits.

Current operating instructions live in [the operations runbook](../operations.md), not the imported historical snapshots. Current verification is recorded separately in [verification](../verification.md).

The current virtual-software contract is [virtual system acceptance](../virtual-acceptance.md). Its current-run commands, failures, and review results are recorded separately as S16 in [the evidence register](evidence.md).

For the next task, read [the GitHub Actions and Nix cache handoff](../handoff/ci-nix-cache.md). [Production-readiness gates](../production-readiness.md) define the release work beyond CI. S17 records this documentation-only planning pass, not a new hosted or application result.

S18 implements that handoff using signed GitHub Releases rather than an external
cache account. Its [CI/cache record](../evidence/ci-nix-cache-20261114.md) retains
hosted identities, actual export failures, source review, operator permissions,
and the independently scoped publication/consumption results.

P0 is complete for the [named delivery receipt](../evidence/ci-nix-cache-20261114-receipt.json).
Use [the download procedure](../binary-cache.md) for the real cache values. Next
is P1's actual agent media path, not another publication run or deployment.

The separate [S19/E15 control-plane follow-up](evidence.md) tracks unmerged PR #2
and supersedes S16's ordinary Stop limitation only on that candidate. Start with
[its review record](../evidence/urgent-control-review-20261114.md) before resuming
control-plane work; do not infer that main or deployed services contain it.
