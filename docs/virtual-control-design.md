# Virtual control and recovery

The urgent-control implementation is an experimental candidate in
[PR #2](https://github.com/tenzyu/splitcast/pull/2), not deployed runtime.
The [review follow-up](evidence/urgent-control-review-20261114.md) distinguishes
fixed defects from tested limitations. The PR records final-head verification.

## Intent, acknowledgment, and completion

Stop admission is independent of the ordinary work queue. After authorization,
Stop advances the admission generation and retains one coalesced intent ticket.
Previously queued activation cannot borrow the newer generation after cleanup.

The HTTP connection waits at most 50 ms for an already available completion.
Otherwise it returns `202` with `stop: "requested"`, `outcome: "accepted"`, and
`cleanup: "pending"`. This wait does not depend on a normal-command busy flag.
Preparation, `tick`, lease/health maintenance, and failure compensation can all
block the effect owner without making the acknowledgment wait for that work.
Acceptance survives caller disconnect and does not prove resource release.

`200` with `stop: "completed"` describes verified cleanup. An incomplete attempt
retains Stopping and unresolved obligations. Its outcome is available as a
`409` response if completion arrives during the acknowledgment window, or as
`stop: "incomplete"`, `outcome: "unknown"` through subsequent status observation.
Status HTTP 200 means a successful observation, not successful cleanup.

## Visibility while the owner is blocked

While Stop is pending, both authenticated `GET /api/status` and POST `status`
bypass ordinary execution and read canonical control metadata. They report
Stopping and requested intent. `children: null` and
`resource_observation: "unknown"` explicitly avoid fabricating an empty worker
list or fresh physical-resource observation.

After 15 seconds without confirmed resolution, the pending observation reports
`cleanup: "unknown"`, `outcome: "unknown"`. The intent and obligations remain
owned. This is an observation budget, not proof of cancellation or a kernel I/O
deadline. Normal detailed status resumes after reconciliation; completion
requires Idle with no unresolved ownership. No durable operation registry is
introduced.

## One effects owner and one authority source

Agent alone owns configuration, Workers, RPC sequencing, the role lock, and
lifecycle transitions. SessionControl shares only canonical phase/authority
metadata, a generation, the active preparation owner, and the current/latest
Stop result. It does not own a second Agent or cloned processes.

Authorization, lease checks, Stop revocation, and Ready/Running commits use the
same short mutex. No copied authorization snapshot needs synchronization.
Operation checkpoints observe revocation, abandonment, deadlines, and expiry.
OBS and peer writes check the generation while performing one nonblocking
socket syscall under that mutex. Capacity waits and acknowledgment reads hold
no metadata lock. An earlier partial transmission or entered process launch
remains a possible external effect requiring compensation.

The listener checks expiry independently of Agent progress. Owner-driven
expiry uses the same revocation before entering Stopping. Inbound heartbeat
renews only a live matching Ready, Running, or Degraded lease. Preparing keeps
its initial lease. Outbound capture heartbeat and health work remain with the
effect owner, so prolonged stalls can fail closed through production expiry.

## Remote completion and uncertainty

Capture records its remote owner with the first positive Prepare socket write,
under the same mutex as generation revocation. Cancellation before dispatch
does not invent a remote obligation. Even a transmitted HTTP prefix retains
uncertainty. Lost Prepare or Start responses do not erase that obligation or
permit blind replay.

Remote Stop may acknowledge intent before completing cleanup. Capture observes
that accepted intent with read-only, owner-scoped status requests inside its
existing cleanup budget. It does not retransmit Start/Prepare or automatically
restart incomplete cleanup. Only matching `owner`, `stop: "completed"`, and
`state: "idle"` confirm release. Bare Idle, another owner's proof, malformed
responses, or lost replies retain uncertainty.

Peer Start cannot implicitly prepare Idle. A stale transmitted Start therefore
cannot recreate a stopped session. An unknown Prepare still in transmission
cannot be declared absent merely by observing Idle. The latest owner completion
is in memory, not a durable cross-host fencing or exactly-once protocol. Peers
lacking the completion fields cannot confirm release through legacy HTTP 200.

## Bearer principal and owner constraint

The bearer token authenticates the operator principal. `_owner` is an optional
peer-session constraint, not a separate credential. Stop requires the matching
held production owner, including after lease expiry. Expiry still revokes
activation and heartbeat renewal. It does not remove authority to release
already-held effects or retry incomplete cleanup. A past owner cannot revoke a
newer owner's generation. An authenticated Stop without `_owner` retains local
emergency authority over a peer-owned session.

Capture possesses the production bearer for peer RPCs and can intentionally
omit `_owner` to exercise that same emergency authority. This is an existing
trust-boundary decision, not isolation between local and remote token holders.
Clients or browser tabs sharing a bearer are not mutually isolated. This trust
model requires trusted operator clients and controlled access to the loopback
service. Host, Origin, framing, and body limits remain enforced before any
control mutation.

## Ephemeral observation and replay limits

Stop has no per-intent correlation ID or durable result registry. Status shows
the current pending intent or the latest completion while Idle. A new Prepare
clears that completion. Another session's later completion can replace its
owner proof. An unowned status result cannot identify an earlier caller's Stop.

For owner-scoped cleanup, a lost reply can be reconciled through read-only
status only while matching owner, completed Stop, and Idle remain observable.
A wrong owner, a refusal, or overwritten metadata is unknown, not proof of
release. Matching owner Stop remains retryable while that owner holds unresolved
effects, even after expiry. No retry may cancel a different owner's session.

Operator clients must coordinate so another client does not Prepare before the
first client observes Stop completion. If that window was missed, pause further
commands and reconcile the currently held session under explicit operator
authority. Do not infer the old result or blindly stop a newer session.

The local generation fence covers admitted and queued work, not an arbitrary
cross-host message still in transit. A duplicate Prepare for the current owner
can be idempotent; a later Prepare admitted from Idle can create fresh effects,
even with a previously used owner ID. Completed Stop proves release of the held
session at reconciliation, not rejection of all future Prepare messages.
Supported clients issue one Prepare attempt, never automatically replay it after
uncertainty, and use fresh owner IDs for fresh authorizations. Concurrent or
reordered external replay is outside this in-memory protocol's safety envelope.
An unresolved transmitted Prepare remains Stopping until release is proved.
If no owner-bound proof is available, quiesce the peer request sources and
inspect and release held effects under coordinated operator control before a
fresh session. Plain Idle or an agent restart alone cannot prove that absence.

## Bounds and shutdown

Once a complete authenticated request can be parsed, Stop admission is
independent of the ordinary work queue and effect-owner progress.
The existing total cap remains eight connection workers. Six ordinary response
waiters and five queued requests leave ingress capacity under ordinary queue
pressure. Slow headers, asset readers, or hostile authenticated control can
still exhaust finite transport capacity. The local 500 ms acknowledgment target
is not socket-level DoS immunity or a real-time scheduling guarantee.

Connection workers explicitly shut down completed sockets. ListenerOwner keeps
the shutdown handles and joins all threads, but its later reap pass does not
gate response EOF. Shutdown closes admission, rejects queued work, requests
cancellation, reconciles effects under an independent cleanup budget, and joins
owned connection work before returning.

Shutdown cleanup reports an error if release was not confirmed. Agent Drop is
best effort and can repeat an incomplete owner-scoped release attempt, including
restoring its owned tunnel for reconciliation. It never replays Start or Prepare.
That later attempt does not turn an earlier error or lost reply into externally
observed completion. Listeners may already be closed, so inspect resources and
ownership rather than assuming Drop supplied a terminal acknowledgment.

Cleanup retains its 15-second cooperative effect budget plus worker/group
teardown and mandatory reaping. Filesystem or kernel stalls and escaped
descendants prevent an absolute wall-clock guarantee. No result authorizes
physical capture, real hosts, privileges, public streaming, or deployment.

## Design choice and historical evidence

An independent authorization latch was chosen over an owner-drained urgent
mailbox requiring every blocking transport boundary to become resumable. A
second queue alone would not admit authorized intent while the owner blocks.
The implementation keeps synchronous effect ownership and adds no executor or
dependency. It also rejects a flag-only cancellation patch, which would not
fence queued work or final activation writes.

The earlier S16 implementation established queue expiry, cooperative server
shutdown, tracked sockets/joins, absolute HTTP budgets, and unknown outcomes.
Its 124-test checkpoint and three local components remain historical evidence,
not results for this candidate. Settings durability, actual agent media-path
integration, browser ownership, and physical acceptance remain separate gates.
