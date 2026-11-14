# Virtual control and recovery

## Problem

The local test matrix must establish control, transport, state transitions, and recovery without claiming physical capture acceptance. The current server serializes acceptance with effects, loses connection join handles, and leaves commands executable after their answer wait expires.

## Usage and contract

The CLI continues to accept the existing commands. A successful response describes a completed command, not an exactly-once transaction. A client-side transport failure after sending command bytes means the outcome is unknown. Clients must inspect current status and use explicit Stop rather than automatically retry Start.

Queue expiry and cancellation race atomically with execution. Only cancellation before execution guarantees no effects. After execution starts, cancellation is cooperative and does not prove rollback. Remote ownership and failed cleanup obligations remain intact.

The library server accepts per-instance cancellation. The binary owns its signal adapter. Server completion requires closure of accepted sockets and joining every connection thread. Cooperative deadlines bound ordinary socket and readiness waits. Kernel-stuck processes, filesystem operations, and escaped process groups prevent a universal hard shutdown bound.

## Shape

One thread owns the mutable Agent and its Session, Workers, and role lock. A listener owner tracks a bounded set of connection sockets and join handles independently of command execution. A request ticket contains a mutex-protected queued, executing, cancelled-before-start, or finished state. Its short critical section decides whether execution may begin. No ticket lock spans effects or I/O.

Absolute HTTP write deadlines cover the header, body, and flush. Every partial write uses the remaining budget. Nested command effects check cancellation between calls. Cleanup has its own budget and does not inherit caller cancellation.

## Synthesis decision

Three independent sketches were compared against queue cancellation, unknown outcomes, shutdown ownership, implementation size, and observable local tests. The cross-judge preferred the listener-owner design for its explicit unfinished-shutdown ownership. The implementation retains that separation and Sol's atomic ticket transitions. It does not introduce Luna's operation registry, query API, retry keys, or retention policy.

The cross-judge's proposed Pending server API is deferred. This change preserves mandatory joins and reports the absence of a universal hard return bound instead of returning while work remains owned by a hidden background thread. The maintainer must not describe a measured local shutdown test as proof against uninterruptible kernel I/O.

Settings durability after rename is outside this unit. The existing review finding remains open rather than adding an unrelated storage migration.

## Alternatives

A resumable nonblocking Agent state machine would interleave Stop with every effect, but would rewrite preparation and OBS configuration. An operation registry would support querying historical outcomes, but would add protocol and retention obligations. Neither is required to prevent expired queued execution or represent transport ambiguity honestly.

## Verification and remaining limits

Run slow-consumer deadlines, queue cancellation races, shutdown during readiness, connection closure, repeated same-process server use, and two real agents with external-service stand-ins. Run generated-media transfer separately with delay and disconnect. These checks do not establish actual SSH encryption, real OBS decoding, GPU contention, Direct Scanout, capture permissions, physical audio provenance, synchronization, or gameplay performance.

The Nix runtime gate remains false and autostart targets remain empty. No deployment or physical-device operation is authorized.

## Implementation reconciliation

The integrated library signature is `agent::serve(Config, Arc<AtomicBool>) -> io::Result<()>`. The caller requests shutdown by setting the per-instance flag. The binary owns signal installation, restoration, and a joined signal bridge. `server::ListenerOwner` owns all accepted socket shutdown handles and connection joins. `request::RequestTicket` owns the queue-to-execution transition. These implementation choices were accepted by the coordinating session after reading the actual diff.

The defaults are five seconds of queue residence and 120 seconds per executing operation. They are finite safety policy, not measured startup suitability. Helpers, peer calls, receive observations, and OBS RPCs use operation cancellation or remaining deadlines. Nested Prepare within Start inherits the same operation. Cleanup replaces caller cancellation with a separate 15-second effect budget. Process teardown retains per-worker TERM grace and mandatory leader reaping, so the cleanup budget does not impose one aggregate hard wall-clock bound.

Ordinary Stop still queues behind active Prepare. The test explicitly observes that restriction before requesting per-instance server shutdown. This change establishes cooperative whole-server shutdown, not immediate emergency-command preemption. Lease maintenance remains serial and is not a real-time deadline guarantee.

Client error handling now labels transmission failures, lost replies, incomplete responses, and unrecognized HTTP statuses as unknown. A complete explicit `not_executed` response preserves the proven pre-start distinction. No redirect is followed and untrusted peer error text is not echoed.

The reusable verifier adds `verify-virtual` and refuses a required opt-in check that selected no tests. Its evidence JSON exemption preserves recorded Nix identities without exempting runnable shell files. The original evidence remains unchanged.

The first primary two-agent run passed before server integration. The final integrated matrix now passes all 14 steps with 124 ordinary tests and three separately executed local components. Independent lifecycle reviews returned no verified product defects in their scoped snapshots. Three review-only settings-path assertions were corrected and rerun with positive and negative controls. See [the verification record](evidence/virtual-system-20261114.md) for the preserved original instrument, correction, and limits.

## Principles that changed implementation choices

- Model the Domain and Type System Discipline put queue and execution authority in a private phase enum and a typed operation context instead of disconnected flags in each caller.
- Foundational Thinking captured the baseline and built observable checks before changing server ownership.
- Build the Lever added `verify-virtual` to the existing command recorder rather than leaving the evidence as one-off shell steps.
- Prove It Works and Test Behavior, Not Implementation selected actual agent status, settings-file effects, worker PID disappearance, released ports, and full media decoding as assertions.
- Sequence Work into Verifiable Units kept the HTTP, verifier, server, and independent test work separately checkable before integration.
- Separate Before Serializing Shared State assigned independent worktrees to writers and separate fixture directories to concurrent test instances. The Agent remains a single mutable owner.
- Fix Root Causes replaced timeout renewal at each write and rejected empty test selection in the verifier itself. It did not rewrite stored evidence to hide a failed check.
- Laziness Protocol rejected an operation registry, a new executor framework, and unrelated settings-durability changes.
