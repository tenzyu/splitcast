---
name: splitcast-context
description: Trace SplitCast design decisions, incidents, disputed acceptance claims, or ambiguous implementation behavior to the tested revision and evidence.
---

# SplitCast context

Use the **working-tree** [capsule](../../../MEMORY.md) and [topic index](../../../docs/memory/index.md). Reuse them if already loaded. The Wiki can postdate the implementation it describes.

Jump to the linked heading with `rg -A 8 '^### Heading' path`. Follow its primary links at their own revisions, e.g. `git show REV:path | sed -n '40,60p'` for a link ending in `#L40-L60`. Batch ranges shared by several questions. Once a rule and its receipt answer the question, finish the lookup.

For polling, report each reader’s eligibility, concurrency limit and interval or backoff where specified. For a lifecycle race, trace work already queued through the transition and check who proves release.

Before promoting a claim, identify the revision, role, environment, evidence class (source, observation, inference or unknown), and remaining uncertainty. Cite the exact symbol, test, job or dated receipt. A later pass can narrow an earlier failure. Physical acceptance requires the corresponding physical observation.

Treat offline PR identities as dated snapshots. Check GitHub only for a question about current state or a moving dependency. For a correction, follow the [update protocol](../../../docs/memory/update-protocol.md), retain the original failure and record what changed. The existing [Wiki scan and opt-in Dreaming workflow](../../../docs/convergence.md#wiki-maintenance) handle maintenance.
