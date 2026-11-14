# Urgent-control candidate evidence

**Scope: historical implementation checkpoints.** Fresh publication-branch
verification and admission measurements belong in the review PR. Earlier
measurements do not establish final-tree acceptance or production readiness.
Exact attribution of the historical timeout remains unresolved and is not a
publication blocker. Current safety, liveness, security, and test failures are.

## Source and environment

The untouched baseline is `2f65bcbe0d7daeee83e4a5cfa1309b9051941d76`.
The original private branch is `fix/urgent-control`. Local commits `a66dae8` and
`7a54afa` remain intact. Subsequent corrective changes are not attributed to
those commits' results. Final validation uses `nix develop` in the candidate
worktree, not another checkout's direnv environment.

## Proven defects and causal boundaries

1. Baseline Stop queues behind Prepare. A readiness HTTP request was observed
   and its reply withheld. Stop received no response before the three-second
   client timeout. This regression failed before production changes.
2. The first candidate admitted intent but acknowledged immediately only when
   `executing` was true. Only normal `execute_work` set that flag. A deterministic
   test runs `tick -> exited worker -> fail -> cleanup_owned -> remote Stop`,
   withholding the remote acknowledgment. Wrong-owner Stop is refused without
   revocation. Authorized intent is observed by the original Operation, but the
   HTTP read times out with **zero received bytes** while cleanup remains
   blocked. This validates the maintenance/failure-cleanup acknowledgment gap.
   The correction removes the flag and independently limits the completion
   wait to 50 ms before returning `202 requested`.
3. A separate test pauses listener reaping after a response completes. Before
   explicit socket shutdown, the client receives the entire `200` header and
   JSON body, then times out waiting for EOF because the listener holds a socket
   clone. Explicit connection-worker shutdown removes that dependency. This
   is not evidence that every earlier timeout had this cause.

The historical two-agent failures returned read-side `WouldBlock/EAGAIN` after
30 seconds. Their OBS logs reached four StartRecord calls and continued health
observations. They did not retain received response bytes or a complete owner
activity trace. Assigning either proven defect to those particular episodes
remains a hypothesis. Later diagnostic clients preserve response prefixes and
owned thread wait information on failure.

## Executed checkpoints, not final-tree acceptance

| Source or check | Result |
| --- | --- |
| Baseline `nix develop -c cargo run -p splitcast-xtask -- verify` | All 11 steps passed. |
| Baseline `nix develop -c cargo test -p splitcast-rs-core --test two_agents two_actual_agents_control_failure_and_explicit_recovery --locked --offline -- --exact --nocapture` | One original recovery test passed, 24.25 s. |
| Initial delayed-Prepare regression | Failed before implementation, Stop response timeout. |
| Intermediate native unit run | Failed because a fixture tried to commit Ready with an expired lease. The fixture was corrected; expired preparation still has a negative test. |
| Intermediate workspace and no-default-feature runs | Two-agent Stop read timeouts retained. Isolated reruns do not erase them. |
| Intermediate verifier `1794651577020124242` | Failed Clippy: argument count and single-element loop. Both corrected without suppressing lints. |
| Clean-commit verifier `1794652464237361004` | Failed an invalid implicit-Start fixture: remote Start from Idle is intentionally refused. The fixture now uses local authority for implicit Start. |
| Verifier `1794652791355305654` | Ordinary tests passed; no-default-feature recovery timed out. |
| Verifier `1794654001360605850` | All 11 steps passed before the latest corrective changes. Not final acceptance. |
| Maintenance/failure-cleanup acknowledgment test | Failed with admitted intent but no HTTP bytes; passed after independent acknowledgment budgeting. |
| Delayed-reaping EOF test | Failed with a complete HTTP response but no EOF; passed after explicit shutdown. |
| Corrective native unit run | 103 passed, one opt-in test ignored. |
| Corrective HTTP integration run | 20 passed. |
| Corrective two-agent run | Five passed, one subprocess entrypoint ignored. Further coverage was added afterward. |
| Frozen tree `0cfb141`, verifier `1794657539558961121` | All 11 steps passed. Default and no-default-feature variants each passed 142 tests, with five explicit opt-ins/entrypoints ignored. |
| Same tree, extended verifier `1794657691496755706` | 13 of 14 steps passed. Ordinary tests failed at the pre-existing OBS geometry fixture's released-port assumption; no-default-feature and all three opt-in local components passed. |

Diagnostic runs on the earlier candidate included isolated recovery, parallel
two-agent cases, syscall tracing, and repeated full no-default-feature tests.
Those reruns passed and did not recover the missing historical causal trace.
They are not a substitute for final-tree verification.

The original recovery test function body is byte-for-byte identical to the
untouched baseline. Its helper observes an accepted Stop using read-only status
until verified completion. It does not resend Stop, retry incomplete cleanup,
increase the original deadline, remove worker-PID checks, or weaken the
recording-versus-streaming assertions.

The OBS geometry fixture reserved an ephemeral port, dropped the reservation,
then required a later bind of that same number to succeed. Another test could
legitimately acquire it. The fixture now follows the adjacent configuration-only
test's `config(0)` pattern, so `launch_args` performs the ephemeral bind itself.
No OBS process starts, all geometry assertions remain, and the occupied-port
negative test still holds a real listener and must be refused. This is a fixture
TOCTOU correction, not a change to the production occupied-port guard.

## Publication verification procedure

Freeze the source and record its SHA, dirty status, Cargo.lock, toolchain, and
source-content hashes. Run these from that same candidate tree:

```sh
nix develop -c cargo run -p splitcast-xtask -- verify
nix develop -c cargo run -p splitcast-xtask -- verify-virtual
bash tools/measure-stop-admission.sh
```

The measurement driver requires five real delayed-Prepare cases and five real
failure-cleanup cases, with acknowledgment and cleanup observations present.
It rejects empty test selection. Timing starts before TCP submission and ends
after the complete acknowledgment. Cleanup timing additionally verifies Idle,
empty ownership, no output, and actual owned PID disappearance. The Prepare
case includes a deliberate 600 ms reply hold. These are debug-build loopback
acceptance samples, not production percentiles or physical-media benchmarks.

The earlier five admission samples, 16.2–20.4 ms, preceded later changes and
must not be presented as measurements of the final tree.

## Evidence preservation and publication

Raw logs and the original chronological record were copied and SHA-256 checked
under ignored `artifacts/verification/urgent-control-private-1794656946915325904`.
The original raw directory remains local but is untracked and ignored. No
failed output was deleted or changed into a pass. Six orphaned stand-in workers
left by two forced fixture-controller teardowns were identity-checked against
their exact fixture namespace and entrypoint, terminated, and verified gone.
This does not establish production crash containment.

A publication scan covered 64 historical/worktree evidence file instances and
found no matching absolute-home, user-runtime, credential-prefix, or bearer
material patterns. That scan is not a general secret-detection proof. Raw
machine/process information and verbose tool logs remain private regardless.
Earlier local commits containing raw logs are not rewritten and must not be
pushed as a shortcut to publishing the reviewed, minimal result.

No Nix runtime gate, autostart target, device, private host, privileged helper,
or public stream was enabled. Kernel-stuck I/O, escaped descendants, durable
outcome history, and deployment containment remain outside these local tests.
