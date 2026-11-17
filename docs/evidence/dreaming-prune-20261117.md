# Dreaming / OKF pruning audit — 2026-11-17

Scope: Markdown files (`**/*.md`, including root Markdown) in `tenzyu/splitcast`. Reference point is the **existing PR #10 worktree before pruning**, commit `b664f86932be9eb4bca7a5e18d5b1427c55214f4`. This is not a token-counter measurement and not a live application test.

## Measured reduction

| Metric | Before | After OKF/prune pass | Difference |
| --- | ---: | ---: | ---: |
| Active-tree Markdown files | 80 | 45 before this audit receipt | 35 fewer |
| UTF-8 bytes from Git tree blob sizes | 727,859 | 329,864 before this receipt | **397,995 fewer (54.68%)** |
| All preserved original files in named archive branch | 80 | 80 | none destroyed |

This audit receipt itself adds Markdown bytes; CI docs changes add Python/shell only. Final counts must be remeasured from the PR head; the target is active-tree Markdown `<=363,929` bytes, i.e. at least 50% less than the declared baseline.

## Information-preservation strategy

- Preserve original pre-prune main at named Git branch [`archive/pre-dreaming-20261117`](https://github.com/tenzyu/splitcast/tree/archive/pre-dreaming-20261117), based on `ed7fe60`. The original PR #10 memory updates are retained in that PR's ancestry; use its original commit `b664f869` for their exact bytes. Git object identity and URLs, not re-summarized prose, preserve forensic detail.
- Prune 37 superseded/verbose active-tree Markdown files (326,081 bytes), including historical copied narratives, old CI/cache handoffs, completed audit TODOs and dated duplicate reports. Actual code, non-Markdown receipts, manifest and current operations are retained.
- Keep live-tree [ADR performance-first](../history/source/docs/graphics-streaming/ADR-2026-11-07-performance-first.md), [architectural decisions](../history/source/docs/ARCHITECTURE_DECISIONS.md), and [approved decisions](../history/source/docs/graphics-streaming/DECISIONS-2026-11-06.md). No ADR deletion, rewrite, or interpretation as current deployment approval.
- Distill long [semantic review](../design-review.md), [verification](../verification.md), [startup](../../MEMORY.md), and [working state](../memory/session-state.md) into small retrieval notes, with stable archive links to full originals. [Decision IDs](../memory/decisions.md) and [evidence IDs](../memory/evidence.md) remain independently searchable.
- Add [OKF v0.2 index](../memory/index.md), frontmatter on each concept, [Dreaming process](../memory/dreaming.md), [change log](../memory/log.md), and a deterministic CI [Markdown/OKF checker](../../tools/ci/check_docs.py).

## Integrity / verification boundaries

Deterministic acceptance checks are: (1) OKF v0.2 concept `type` and special index/log syntax; (2) every active Markdown relative link resolves (including links repointed into the named archive); (3) ADR and historic decisions survive; (4) D01–D18 and E/S register IDs remain available; (5) source/risk distinctions and explicit authority boundaries survive; (6) current source code and runtime gates are unchanged; (7) final PR CI is independently checked.

**Not run by this Dreaming pass:** physical media, agent command, actual Twitch subscription, game OFF/ON comparison, local Cargo/Nix verifier or deployment. CI is the hosted execution path for new code (docs checker). No Anthropic-style memory retrieval benchmark on SplitCast has been run, so `no loss of downstream answer quality` is an architectural preservation objective, **not** an empirically proven model score.

Primary methods: [Anthropic agent memory](https://www.anthropic.com/research/components-of-agent-memory) and [OKF v0.2](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md). Historic run outcomes remain tied to their exact sources; the new Wiki cannot silently upgrade them.
