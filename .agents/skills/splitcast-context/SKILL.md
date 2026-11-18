---
name: splitcast-context
description: Trace SplitCast design decisions, incidents, disputed acceptance claims, or ambiguous implementation behavior to the tested revision and evidence.
---

# SplitCast context

Start with the [startup capsule](../../../MEMORY.md) and [topic index](../../../docs/memory/index.md). Reuse them if already loaded. Open just the matching section. Follow its pinned source with `git show REV:path` or `git grep -n PATTERN REV -- path`. Read the rule **and** the observation that exercises it. For a polling incident, look for the scheduling bound as well as the trace. For a lifecycle race, check work admitted before the transition as well as the final owner proof.

Before promoting a claim, identify the revision, role, environment, evidence class (source, observation, inference or unknown), and remaining uncertainty. Cite the exact symbol, test, job or dated receipt. A later pass can narrow an earlier failure. Physical acceptance requires the corresponding physical observation.

Treat offline PR identities as dated snapshots. Check GitHub only for a question about current state or a moving dependency. For a correction, follow the [update protocol](../../../docs/memory/update-protocol.md), retain the original failure and record what changed. The existing [Wiki scan and opt-in Dreaming workflow](../../../docs/convergence.md#wiki-maintenance) handle maintenance.
