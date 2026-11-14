# Open risks

Recorded 2026-11-13. These are unresolved historical gates and extraction questions, not newly reproduced standalone defects. Evidence IDs resolve in [the register](evidence.md).

| ID | Unknown or risk | Evidence needed to close it | Sources |
| --- | --- | --- | --- |
| R01 | End-to-end GSR capture, production OBS decode, screenshot, recording, synchronization, and competitive non-regression are unaccepted. The historical KMS helper was absent. | Separately authorized capture with a reviewed narrow privilege boundary, real media checks, and controlled game-only versus capture comparisons. Preserve a blocked result rather than substitute desktop capture. | S06 completion gates, S08 limits, D01, D06 |
| R02 | Controller SIGKILL, process-group escape, uninterruptible kernel I/O, and total remote failure may outlive ordinary cleanup deadlines. | Service-cgroup forced-death and failure trials with independent descendant, socket, and remote-publication observations. State which dependencies cannot guarantee immediate cleanup. | S06 worker/audio rows and review holds, S07 limits, D04 |
| R03 | Speech delivery/playback, EventSub, persistent replay, shared camera/TTS transactions, arbitrary graph editing, and full recovery are unfinished or refused. | Feature-specific observable contracts and actual command-path tests. Do not equate a successful synthesis component with delivered speech. | S06 implementation table, D02 |
| R04 | Progress counters do not establish bounded frame age. TCP head-of-line blocking, clock correspondence, queue behavior, drift, and aggregate shutdown latency remain unmeasured. | Stage-specific progress and age instrumentation, bounded-load and loss tests, measured clock relationship, and authorized transport comparisons. Do not subtract unrelated host-local timestamps. | S04 transport/shared resources, S07 limits, S08 limits, D03, D06 |
| R05 | A stock OBS build is not proof that effective listeners and network policy preserve the old loopback-only security property. | Local socket inspection and independent IPv4/IPv6 ingress probes under the actual selected deployment policy, plus authentication and persistence checks. No listener or firewall change is authorized by this page. | S03 stock OBS contract, S06 permissions, D06 |
| R06 | Basic local extraction portability is verified by E07–E10, but the new synthetic SSH parameterization has not been run against authorized hosts and public-URL consumer use remains pending. | Human publication, input URL replacement, repeated consumer gate evaluation, and a separately authorized portable-host synthetic run. Keep those scopes separate from successful local builds and scans. | S00, E06–E10, D07, D08 |
| R07 | Native web-model tests and a WASM artifact do not prove live browser behavior. A GPUI client is only a stated future-client requirement. | Browser interaction through the real API and explicit implementation evidence for any desktop-client claim. | S05, S06 views row, D02 |
| R08 | Original private archives, full local logs, and generated recordings may not remain available. Sanitized copies cannot reproduce their original hashes. | Preserve original Git object identities and label report-only claims. Register a raw artifact only after actually locating it and recording its identity and scope. | Source register, S06, S08, S09 |
| R09 | Whole-server cancellation/task joining, abandoned-request outcomes, structured error taxonomy, public invariant-bearing fields, adjacent FFI proofs, and browser request ownership remain incomplete. | Focused implementation and negative-path tests described in semantic review findings F02–F07. Source extraction and green ordinary tests do not close them. | S15, D09 |
| R10 | Full consumer flake evaluation fails at an invalid Firefox theme derivation. | Repair the consumer environment or unrelated derivation in a separate task and rerun the full check. The exact failure also reproduces at untouched EXTRACT; targeted SplitCast config/gate checks pass. | S14, E09 |

Historical approval covered disabled partial publication only. Capture enablement, Rust service selection, host activation, a privileged helper, external listeners, and public streaming require distinct current authorization. No present host state is inferred from a repository boolean.

## 2026-11-14 virtual-software follow-up

S16 narrows R09 with implemented queue-expiry cancellation, explicit unknown outcomes, per-instance shutdown, tracked connection joins, and focused negative-path checks. It does not close the entire risk. Ordinary Stop remains serialized behind Prepare, lease progress is not real-time, and a universal hard shutdown bound is not claimed. F04 through F07, settings durability, and durable command outcomes remain separate work.

R04 now has a generated-only delayed loopback TCP transfer, disconnect cleanup, and a freshly decoded recovery recording. That evidence does not cover the actual agent RTMP and RTSP media path, frame freshness, clock correspondence, or physical latency. Those software integration checks can proceed without hardware-performance acceptance.

R01, R02, R05, and R07 retain their physical, deployment-containment, network-policy, and browser acceptance boundaries. D10 separates them from the local software development gate rather than declaring them passed or making them prerequisites for unrelated software progress.

## 2026-11-14 delivery planning follow-up

S17 does not close a runtime risk. [Production-readiness gates](../production-readiness.md) map the outstanding work to release evidence. [The next-session handoff](../handoff/ci-nix-cache.md) owns the immediate CI/cache task.

- R11. The repaired source is uncommitted while HEAD remains `aa86fe0`. A hosted checkout or new worktree at HEAD alone would test an older artifact. Close with a deliberately reviewed commit and a hosted run for that exact source identity.
- R12. Hosted CI, cold-runner Cargo/WASM preparation, current native/WASM Nix packages, cache publication authorization, and fresh-consumer substitution remain unverified. Close with hosted run and output identities, scoped credentials restricted to trusted revisions, and a no-fallback-build consumer result from the intended cache. Cache availability alone does not close R01 through R10.

The source-inspected sandbox shebang and toolchain/cache risks in the handoff are hypotheses to reproduce, not newly proven product failures. Do not weaken assertions or substitute a different OBS backend to hide an infrastructure failure.

## 2026-11-14 PR #2 control-plane follow-up

On `review/urgent-control`, implementation `5125799` supersedes S16's ordinary
Stop-behind-Prepare limitation with listener-side admission and generation
fencing. Matching expired owners retain release/retry authority, not renewal or
activation. Remote obligations begin at positive socket writes. See
[S19/E15](evidence.md) for red/green evidence and final-head checks in PR #2.
This is not a main-branch or deployed-state claim.

R09 remains open for parser-slot exhaustion, ephemeral completion observation,
cross-host Prepare replay, durable outcomes, and aggregate kernel/cleanup bounds.
R01, R02, R05, R07, actual media-path integration, and F04–F07 retain their
separate gates. Shared bearer clients share operator authority, not isolation.
