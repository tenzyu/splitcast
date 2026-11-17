# Media-plane acceptance

- [x] Read the Principles section of the poteto-mode skill.
- [x] Phase A: Frame.
- [x] Phase B: Design the workflow.
- [x] Phase C: Run the loop.
- [x] Capture the clean baseline and resolve real local-service prerequisites.
- [x] Ground, sketch competing designs, agree, implement, and scrap only if evidence rejects the shape.
- [x] Build an executable generated-media profile and independent output assertions.
- [x] Exercise bad media, failure, cleanup, and explicit recovery.
- [x] Run the complete final ordinary and virtual matrices and the additional real-media gate.
- [x] Phase D: Keep the audit trail.
- [x] Phase E: Verify; original publication completed as PR #3 below.
- [x] Consolidate source-grounded memory after the final verification attempt.
- [x] Publication-sensitive diff check; prior review counterexamples addressed.
- [ ] Final independent approval: human review pending; last provider returned no verdict. Extra review waived for opening the PR by finish steering.
- [x] Published one unmerged review PR after the evidence commit: https://github.com/tenzyu/splitcast/pull/3 at `8486e81`. This is not human approval; see the [2026-11-17 follow-up](pr3-follow-up.todo.md).

## Design arena

- [x] Frame.
- [x] Fan out.
- [x] Cross-judge.
- [x] Pick.
- [x] Graft.
- [x] Verify the selected design against real media; corrected full matrix passes at `42a64e4`.

## Throughput checkpoint

The riskiest unknown is whether real OBS can record this controller's configured RTSP scene on an isolated CPU-rendered display. Resolve that before expanding fault instrumentation. Expected work is one opt-in integration profile, a small production configuration change only if needed, regression assertions, and compact evidence/memory updates. No transport rewrite, hardware access, runtime activation, or control-plane migration is planned.

The original completion predicate required an independently decoded recording from the real controller/MediaMTX/OBS route, a genuinely detected bad-media case, verified cleanup and fresh explicit recovery, and green final regression matrices. Those assertions passed at `42a64e4`; the first final-source virtual attempt remains recorded as failed with unknown cause. PR #3 is now published; its review identified additional oracle gaps tracked in the follow-up, not retroactively tested by the original run. Human review remains pending. No merge or deployment is authorized.
