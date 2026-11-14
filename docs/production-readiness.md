# Production-readiness gates

SplitCast is not Production Ready. [S16/E12](memory/evidence.md) retains its local 124-test software checkpoint. [S18/E14](memory/evidence.md) closes P0 for `d2205d0` on `x86_64-linux`: hosted checks, signed GitHub Releases delivery, and fresh signature-enforced no-build consumption all pass. No delivery result establishes software media-path or deployment acceptance.

Production Ready means that a named release, supported feature set, and supported environment meet explicit behavior and failure contracts. CI success and cache availability are necessary delivery properties, not a substitute for runtime acceptance.

## Declare the supported release contract first

Specify supported host roles, operating systems, architectures, capture backends, audio routing, OBS version, output modes, and client interfaces. The current flake exports only `x86_64-linux`. Do not advertise other architectures based on Rust source portability alone.

A recording-first release with unfinished speech and EventSub refused is a recommended way to reduce initial scope, not an operator-approved product decision. If Twitch, camera, speech, or a browser interface is advertised, each advertised path needs its own acceptance evidence. Do not remove a required feature assertion merely to call the release ready.

Uncertain RPC outcomes are unavoidable after a lost response. A supported release can preserve uncertainty, refuse automatic Start retries, fail closed after restart, and provide a tested reconciliation procedure. Durable operation history is required only if the release promises it. This plan does not reverse D10 by making a general operation registry an automatic prerequisite.

## Required evidence

| Gate | Work remaining | Evidence required before closing |
| --- | --- | --- |
| P0. Reproducible delivery | Complete for `d2205d0` and its pinned `x86_64-linux` inputs. Keep the delivery checks for future candidates. | [S18 receipt](evidence/ci-nix-cache-20261114-receipt.json): hosted validation/builds, signed public release, retained failed attempts and fresh no-build consumption. Different inputs require new evidence. |
| P1. Actual software media path | Connect generated sources to the actual agent RTMP and RTSP receive path. Use real media services rather than fabricated progress counters. | Both agent statuses plus independently decoded video/audio, expected tracks, selected geometry, timestamps or frame-age instrumentation, delayed transport, disconnect, child failure, and explicit recovery. Final recording/output must come from the path under test. |
| P2. Responsive control and cleanup | Define Stop priority, lease progress, queue saturation policy, and one aggregate cleanup budget. Resolve the current Stop-behind-Prepare restriction. | Delayed Prepare, blocked helper/OBS calls, saturated clients, lost replies, SIGTERM, and authorized crash-containment tests. Observe resources and remote obligations directly. Never report Idle or complete cleanup while effects remain unconfirmed. |
| P3. State and trust boundaries | Resolve settings commit ambiguity, validated configuration boundaries, machine-readable error recovery, browser command ownership, and supported protocol compatibility. | Post-rename sync failure, restart readback, invalid payloads, owner conflicts, stale browser replies, duplicate clicks, cancellation, and authorization tests through supported interfaces. Document any deliberately unsupported stable-library contract. |
| P4. Operational reliability | Define health signals, failure escalation, upgrade/rollback procedure, secret handling, dependency policy, and support diagnostics. | A finite, declared soak workload and fault matrix with resource/queue observations, bounded retries, leak checks, actionable logs, and a tested rollback to the approved prior artifact. Set acceptance thresholds before evaluating results. |
| P5. Target-environment acceptance | Accept the actual capture and production environment for the supported release. | Separately authorized capture permissions, selected-device provenance, Direct Scanout behavior, GPU contention, physical audio, actual OBS preview/output, synchronization, gameplay non-regression, listener isolation, and service-cgroup containment. |

P0 through P4 can progress without physical hardware-performance approval. Real OBS with generated input and a headless display may also support software integration work, but does not establish physical capture acceptance. P5 is required before calling a particular real deployment accepted. It does not need to block unrelated software changes.

No program can guarantee a hard stop of uninterruptible kernel I/O. Define the observable incomplete-cleanup result and enclosing containment/escalation policy instead of making that impossible promise. Current per-operation deadlines and per-worker reaping are not already a proven aggregate shutdown contract.

## Work in this order

1. P0 is delivered for the named S18 candidate. Preserve [its CI/cache procedure and evidence](binary-cache.md) when changing source or inputs.
2. Close P1 with generated media through the real agent path. A separate TCP recording test is not sufficient.
3. Close P2 and P3 with targeted negative-path and state tests. Keep explicit uncertainty where external effects cannot be confirmed.
4. Close P4 against the declared release workload and failure budgets.
5. Run P5 only after authorization for the selected environment. Enable no runtime or autostart gate merely because earlier gates passed.

The historical [semantic review](design-review.md), [virtual acceptance boundary](virtual-acceptance.md), and [open risks](memory/open-risks.md) remain the supporting records. Closing a row requires a new evidence identity and exact executed results, not a larger test count.
