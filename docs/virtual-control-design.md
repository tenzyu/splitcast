# Virtual control and recovery

The urgent-control implementation is a candidate under corrective validation.
The historical Stop-behind-Prepare restriction is not declared closed until
the same final tree passes recovery, maintenance-delay, and full verification
gates. See the [evidence and outstanding causal question](evidence/urgent-control-20261114.md).

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

Capture records its remote owner before sending Prepare. Lost Prepare or Start
responses do not erase the obligation or permit blind replay.

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
peer-session constraint, not a separate credential. When present, Stop requires
the matching live production owner. An authenticated Stop without `_owner`
retains local emergency authority over a peer-owned session.

Capture possesses the production bearer for peer RPCs and can intentionally
omit `_owner` to exercise that same emergency authority. This is an existing
trust-boundary decision, not isolation between local and remote token holders.
Clients or browser tabs sharing a bearer are not mutually isolated. Host,
Origin, framing, and body limits remain enforced before any control mutation.

## Bounds and shutdown

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
