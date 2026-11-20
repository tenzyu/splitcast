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
lifecycle transitions. SessionControl shares the canonical phase and its
authority, the admission generation, and the current/latest Stop result.

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
its initial lease. The capture Agent still sends its own heartbeats, so
prolonged capture stalls can fail closed through production expiry.

## Production Running supervision

Start checks ingress and OBS output while the Agent owns the effects. On
Running it transfers both comparison histories to one read-only observer.
Each cycle reads one bounded MediaMTX response and authenticates a separate
OBS status connection. The next cycle is scheduled one second after the
previous one finishes. MediaMTX is capped at two seconds and the complete OBS
read at five seconds. The observer replaces one generation-bound progress
snapshot and latches the first terminal fault.

Ordinary admission, I/O and successful completion check the latch. The Agent
also checks for an exited thread or a snapshot eight seconds old. Pending
Stop takes priority. A fault interrupts ordinary work and hands compensation
to the Agent. While it runs, status reports Failed with unknown resource
observation. Final status carries the separately observed OBS inactivity and
exit. Cleanup cancels and joins the observer before reconciling output with a
fresh 15-second cooperative budget.

## Remote completion and uncertainty

Paired Agents require `transport.version = 3` for both media and speech-only
sessions. Authenticated production `routes` includes a `prepare_id` while
production is available in Idle. Capture checks transport compatibility and
appends a fresh 32-character lowercase hexadecimal nonce to that ID to make
its `_owner`. At dequeue, production checks the admission generation,
deadline, Stop and the entire Idle precondition, then commits the owner in
Preparing under the metadata lock before starting effects. In Ready or
Running, the same owner and purpose can observe success again.

Capture records its remote owner with the first positive Prepare socket write,
under the same mutex as generation revocation. Cancellation before dispatch
does not invent a remote obligation. Even a transmitted HTTP prefix retains
uncertainty. Lost Prepare or Start responses do not erase that obligation or
permit blind replay.

Immediate Stop completion requires a production-incarnation `stop_id`, the
held owner, `stop: "completed"`, `outcome: "completed"`, and Idle. On a
`202` acknowledgment, capture retains the admitted `stop_id` and requires it
on subsequent owner-scoped status reads inside the existing cleanup budget.
Bare Idle, changed tickets, another owner's proof, malformed responses, and
lost replies retain uncertainty. On tunnel recovery capture checks
compatibility and uses its held owner for Stop regardless of `prepare_id`.

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

Status shows the current pending intent or the latest completion while Idle.
A new Prepare clears that completion. The Stop ticket lives in memory.

An explicit retry with the matching owner after a lost Stop reply can return
the retained completion or admit cleanup for effects still held by that owner,
including after lease expiry. The first attempt stays unknown until there is
correlated proof. A different owner's session rejects this retry.

Operator clients must coordinate so another client does not Prepare before the
first client observes Stop completion. If that window was missed, pause further
commands and reconcile the currently held session under explicit operator
authority. Do not infer the old result or blindly stop a newer session.

Stop fences admitted local work and invalidates the previous Idle precondition
for peer acquisitions. A delayed Prepare keeps its old precondition across
later sessions and process restarts. A fresh authorization uses the current
Idle revision and a new nonce. An unresolved transmitted Prepare retains its
release obligation. If no owner-bound proof is available, quiesce the peer
request sources and inspect and release held effects under coordinated operator
control before a fresh session.

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
