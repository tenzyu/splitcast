# PR #2 control-plane review follow-up

Recorded 2026-11-14 JST on `review/urgent-control`. Implementation commit
`512579911f47a68f9cf6d9ebd59f502e4d3c52d8` follows reviewed head
`0458502439cde0d660e936a0371434b998709d67`. Later documentation commits do not
change those Rust bytes. [PR #2](https://github.com/tenzyu/splitcast/pull/2)
records the final publication SHA, fresh matrix run IDs, exact admission and
cleanup observations, and new hosted checks. This is not main-branch,
physical-device, or production acceptance.

## A. Expired matching owner cannot retry cleanup: confirmed and fixed

`SessionControl::request_stop` rejected an expired peer even while that peer's
effects remained held in Stopping. The expiry watchdog deliberately does not
restart incomplete cleanup. Stop now checks matching ownership for release,
independently of the still-expired authority to create effects or renew a lease.
Wrong owners and past owners after reacquisition still fail before revocation.

`expired_matching_peer_can_retry_retained_cleanup_over_http` uses an actual
owned receiver and a live private Unix socket that prevents cleanup. It proves
Stopping, expired renewal refusal, receiver PID disappearance, and a retained
live socket. After the socket server exits, the ordinary owner-scoped peer HTTP
Stop must release its file and return matching completed proof. A newer owner's
live receiver and generation survive the old owner's attempted Stop.

Before the fix, the test failed with `control command not executed; no redirects
permitted` on the matching peer retry. After the fix it passes. An earlier test
setup failed at the Unix socket path-length cap, before the fault boundary. The
fixture uses a shorter private directory; that setup failure is not red evidence
for A. Existing expired-lease tests now allow release but continue to reject
renewal and activation.

## B. Remote attempt before dispatch: confirmed and fixed

The old preparation suffix recorded `RemoteAttempt` after route validation but
before the next cancellable RPC. The deterministic pre-dispatch test failed with
`Some(OwnerId("never-dispatched"))` despite proving no peer connection occurred.

`Agent::prepare_remote` now carries a request-local owner through Operation.
`SessionControl::dispatch_prepare` records an obligation only after a positive
nonblocking socket write, under the same mutex as Stop admission. A stale
operation keeps its original generation. No connection, zero bytes, or a refused
pre-dispatch write creates an obligation. Even a partial HTTP prefix does.

`cancellation_before_remote_prepare_dispatch_does_not_invent_an_obligation`
passes and verifies local cleanup. The real-socket test
`remote_prepare_obligation_tracks_socket_bytes_not_bookkeeping` proves both
sides of the write boundary: no post-Stop bytes, no obligation before dispatch,
and retained Stopping after a transmitted prefix. Existing lost-Prepare and
Start-acknowledgment tests retain their unknown-outcome and remote-release checks.

Late Prepare replay is an accepted protocol limitation, not a fixed distributed
fence. `completed_stop_is_not_a_cross_host_prepare_replay_fence` demonstrates a
later authenticated Prepare creating real stand-in workers from Idle, no output
before Start, and explicit owner-bound cleanup. The supported client never
automatically replays Prepare or Start. [The contract](../virtual-control-design.md)
defines completion's scope and coordinated recovery when proof is unavailable.

The state-boundary checks remain concrete: A retains Stopping with a live socket
after failed cleanup; the stalled-failure fixture reports unknown resource
observation before release; the Start-acknowledgment race retains remote ownership;
and the running/delayed-Prepare SIGTERM tests inspect actual worker release.
`serve_bound` reports incomplete cleanup as an error. Agent Drop can make another
best-effort release attempt after the listener closes. That is an owner-scoped
Stop, not Start/Prepare replay or a terminal response to the caller. An earlier
error remains uncertainty even if this unobserved cleanup later succeeds.

## C. Completion observation can disappear: accepted documented limitation

There is no per-intent ID or durable result history. New Prepare clears the
latest Idle completion. Later completion can replace the owner proof. Generic
status therefore cannot reconcile an old caller's intent after another session.

`lost_stop_reply_and_new_session_do_not_fabricate_old_completion_history` reads
only the initial 202 status line, discards the JSON reply, and observes matching
completion using status only. It checks owned PID and worker-port release.
A subsequent session removes old completion, rejects old-owner commands without
affecting new output, and produces only the new owner's proof. Clients must
compare owner/completed/Idle and treat replacement or refusal as unknown.

The first version polled before Stop admission and encountered the legitimate
generation-fenced `503 request superseded by Stop`. Its synchronization now
observes the 202 status line before dropping the reply. No 503 is ignored, no
Stop is resent, and no timeout or original recovery assertion was weakened.

## D. Admission is not parser-exhaustion immunity: accepted documented limitation

The bounded eight-connection cap, six ordinary waiters, five queued requests,
50 ms completion wait, and five-second request parser budget are unchanged.
The existing delayed-Prepare test proves ordinary capacity refusal and queued
activation fencing. The failure-cleanup test withholds remote completion until
the acknowledgment and pending status have been received.

These guarantees begin when a complete authenticated request can be parsed.
Slow clients can occupy every ingress slot. Host/Origin, framing, body limits,
bearer authentication, and owner checks remain required. This candidate trusts
operator bearer clients; it is not isolated multi-tenant or adversarial-client
emergency control. Restricted access and separately authorized service-level
recovery remain operational requirements. No universal latency SLA is claimed.

## Executed focused checks and final verification

All commands use the pinned Nix development environment. Raw output stays in
private local verification storage, including failures.

- Before fixes, each exact A/B test selected one test and exited 101 for the
  behavioral failures quoted above. The commands were
  `nix develop -c cargo test -p splitcast-rs-core --lib TEST --locked --offline -- --exact --nocapture`,
  with `TEST` equal to the full `agent::lifecycle_tests::` name above.
- After fixes, `nix develop -c cargo test -p splitcast-rs-core --lib agent::lifecycle_tests:: --locked --offline -- --nocapture`
  exited 0, 11 passed. Later PID assertions strengthen A without changing its fault.
- `nix develop -c cargo test -p splitcast-rs-core --locked --offline` exited 101
  at C's initial pre-admission observation race. All other selected tests passed,
  including original two-agent recovery. This failure remains recorded.
- After synchronization repair,
  `nix develop -c cargo test -p splitcast-rs-core --test two_agents lost_stop_reply_and_new_session_do_not_fabricate_old_completion_history --locked --offline -- --exact --nocapture`
  exited 0, one passed. `nix develop -c cargo test -p splitcast-rs-core --lib --locked --offline`
  exited 0, 106 passed, one opt-in ignored.

The final documentation-bearing head requires fresh `verify`, `verify-virtual`,
and `bash tools/measure-stop-admission.sh`; results belong to that exact head in
the PR, not to these earlier dirty-tree focused checks. The prior publication
head passed hosted run `43598811247`; it is not evidence for the remediation head.
The original recovery test body, runtime gate, autostart targets, README,
dependencies, and toolchain remain unchanged. Historical EAGAIN attribution is
still unresolved. Kernel stalls, escaped descendants, crash containment, actual
agent media integration, browser ownership, physical devices, and deployment
remain outside this local control-plane acceptance.
