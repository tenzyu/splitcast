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

## 2026-11-15 production-media follow-up

S20/E16 and D14 narrow R01 and R04 with actual production Agent, MediaMTX,
RNNoise/private Pulse, and OBS output at `42a64e4`. The [receipt](../evidence/media-plane-20261115.md)
binds decoded artifacts and negative/recovery observations to that source.
This corrects S16's missing production-only integration claim without closing
the broader risks.

- R04 remains open. Frozen and backlogged frames retain increasing bytes and
  Running status. Independent checks reject them, but no live freshness policy
  is enforced. Next, drive a decode-aware live-health policy with the same bad
  sources and require observable fail-closed behavior. Do not infer freshness
  from the new startup geometry check.
- Capture-Agent and SSH publication, network congestion and clock correspondence
  across hosts remain unverified. Next, use generated capture ingress with both
  actual controllers and a local SSH topology, then impair actual media traffic
  without delaying RTMP control handshakes. Require freshly decoded output and
  both controller statuses. The current queue precedes RTMP ingress.
- Microphone alignment, intended-VAD speech quality, and long-run drift remain
  unknown. Next, inject an independently timestamped microphone marker at the
  intended VAD setting and compare decoded PCM with video markers. The current
  harmonic source uses VAD threshold zero and establishes routing only.
- R02 and R05 remain open for deployment containment and ingress. R09 is narrowed
  by a regression retaining Stopping after unconfirmed OBS finalization, not
  closed for durable recovery, parser capacity, or kernel bounds. No physical,
  GPU-performance, game, device-provenance, camera, or browser gate is closed.

The initial software-mode attempt opened a GPU render node and failed. All
accepted runs hide devices structurally. Retain that contrary observation;
never promote a software setting into a device-isolation guarantee.

AV01 supersedes the initial phase-fit timing claim; the corrected oracle proves
sampled game transitions only. The final-source virtual run also observed an
initial two-agent Prepare connection reset. Its cause is unknown; retain the
failure even if the single permitted retry passes. Do not call it a fixed bug
or a proven load-related flake. See E16 for final regression outcomes.

## 2026-11-17 review and mission correction

S21/S22 and [D15/D16](decisions.md) distinguish the renewed mission from current
implementation. Read [the environment-gap entry](environment-gap.md) next; the
older next-experiment lists above remain proposals in their dated scope.

- R03 includes an independent **required** production-to-capture speech/other
  audio return path, not just optional TTS polish. `agent.rs::prepare` refuses
  speech/EventSub/remote mixing; synthesis component tests are not delivery or
  playback evidence. Define return-path failure isolation and latency separately
  from game interference and media freshness. No protocol/order/number approved.
- R01/R04 remain open despite the [repaired acceptance oracle](../evidence/pr3-review-20261117.md).
  The new gates test decoded cadence, complete-window coverage, consecutive
  absence and source health; they do not add capture-Agent/SSH, network
  congestion, live freshness enforcement, physical audio or gameplay results.
- The original Prepare reset did not recur in twenty focused no-default-feature
  attempts or the implementation-head full verifiers. Both agents' traces,
  worker logs and successful exits are preserved. Cause remains unknown; added
  diagnostics and different concurrency can alter observation. No speculative
  fix or reliability guarantee. On recurrence, preserve those same diagnostics
  and isolate the first demonstrated boundary before editing product code.
- Worker stderr no longer unconditionally enters production logs. No actual
  secret leak was confirmed. Suppression reduces raw diagnostics; a future
  explicit private/bounded/redacted policy remains open rather than expanding
  the logging boundary under an acceptance-test rationale.
- Resource minimization and an optimal transport remain unverified. GSR still
  acquires/encodes on capture host; offloading OBS does not eliminate GPU/memory,
  thermal/power or scheduling interference. Inventory/reuse existing capabilities
  before custom hot paths or more distributed control. No new PoC in this PR.

The original human review remains pending, including the quota-limited review
that returned no verdict. Publication/CI/test success does not authorize merge,
deployment or private-host/device operations. Runtime/autostart gates unchanged.

## 2026-11-17 post-merge initial PoC follow-up

S23 confirms PR #3 merged at `a5e7a22`; the earlier pending-merge wording keeps
its pre-merge scope. S24's [PoC checkpoint](../evidence/poc-20261117.md) records
the new operator instruction and partial evidence, not deployment permission.

- R03 still requires real Twitch-event ingestion, production synthesis, returned
  audio and capture playback. A local fixture command or fixed-text engine probe
  does not close it. No approved usable receiver was found in the inspected scope.
- R01 remains open. Read-only live capture inventory sees the working game/DSO
  condition, but the expected KMS helper is absent and the capture execution route
  is not authorized by read-only SSH. No silent portal/renderer or audio fallback.
- R04 remains open. Eight OFF metadata samples lack a fixed input workload and
  all-frame telemetry; no p99, media freshness, TTS latency or ON comparison follows.
- The existing media profile retains recordings and includes overlong cases.
  Do not run it unchanged under S24's temporary-only, per-recording 30-second,
  aggregate 500-MB, inspect-then-delete contract. A new finite trial must enforce
  limits and release its owned resources before deleting only its own media.
- Readiness S23's audio metadata clients socket-activated inactive shared audio
  services. S24 remote metadata checks service state before connecting and never
  treats shared services as probe-owned. Preserve the earlier side effect.

## S25 convergence correction

Use the [convergence checkpoint](../evidence/poc-convergence-20261117.md) for the
current blocker classification. New Twitch WSS/subscription permission is no
longer missing. The selected channel and suitable existing OAuth configuration
are missing in the inspected scope, and event admission to synthesis still has
no implementation. No scope rejection was observed because no API call ran.

The coarse statement that capture FFmpeg is absent is superseded. Cached
headless FFmpeg exists but lacks Pulse input. GSR/helper and a worker-launch
route remain absent. Native microphone and portal candidates are not accepted
alternatives. DRM-node access alone does not supply the KMS privilege boundary.

The latest whole-suite pass is clean `3c2f913`, not a retrospective pass for the
first convergence run's three reset failures. Preserve those failures and the
outside-session source change. No automatic retries beyond the three recorded
checks are part of this continuation. PoC completion remains open.

## 2026-11-17 Dreaming source/state correction

The sections above are **dated evidence and former handoffs**, not an instruction
to reopen completed CI/cache delivery or to describe merged PRs as unmerged.
[PR #2](https://github.com/tenzyu/splitcast/pull/2) merged at `6349fd6` and
[PR #3](https://github.com/tenzyu/splitcast/pull/3) merged at `a5e7a22`.
Their merge changes source availability, **not** the acceptance scope for R01–R09.
The [Dreaming correction ledger](dreaming.md#current-correction-ledger-2026-11-17)
records the current pointers and preserves the original dated observations.

A later [PR #5 CI run](https://github.com/tenzyu/splitcast/actions/runs/44089362405)
failed `no-default-features` because the expired-owner Stop test required
immediate `completed` after a transport that permits `202 requested`.
The [PR #6 test-only proposal](https://github.com/tenzyu/splitcast/pull/6)
removes the test's cancellation race and validates owner-specific completion
separately. Until the exact amended head passes hosted CI, this remains an
**unverified correction**, not proof that runtime cleanup is fully reliable.
It does not close durable Stop observation, parser exhaustion or R02.
