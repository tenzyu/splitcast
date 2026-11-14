# SplitCast decisions

Recorded 2026-11-13. Dates below are decision or observation dates, not dates of fresh standalone validation. Source IDs resolve in [the evidence register](evidence.md). High confidence means strong documentary support for the stated scope, not production acceptance.

## D01: Streaming remains downstream of gameplay

- Date: 2026-11-05 through 2026-11-13. Adopted contract. Confidence: high.
- Goal: observe authorized output without making gameplay depend on capture, network, or the production host.
- Adopted: separate native media workers under Rust control. Stop or degrade streaming first. Preserve the user's working renderer, input mapping, local audio, and display behavior.
- Rejected: game-side capture injection, renderer substitution, restarting or rescheduling the game to recover streaming, and silent portal fallback.
- Rationale: historical capture-hook crashes occurred in the game's EGL export path. Isolation matters more than moving orchestration to Rust.
- Counterevidence and limits: the crash stack identifies the failing path, not a proven Mesa root cause. KMS avoids that injection but still competes for GPU, memory, and power. Its gameplay impact remains unmeasured.
- Sources: S02 sections 7–9, S04 invariants, S07 goal and rejection rules. The historical OpenGL/native-Wayland scenario is not proof that every OSS user has the same hardware or compositor.

## D02: One Rust command and validation contract serves every view

- Date: 2026-11-13. Adopted historical implementation and operator requirement. Confidence: high.
- Goal: prevent the CLI, TUI, browser, and configuration formats from granting different capabilities.
- Adopted: TOML-first configuration, transitional JSON through the same native validator, portable typed Rust commands, Rust/WASM browser logic, and generated JavaScript loader glue only. A future desktop client uses GPUI.
- Rejected: handwritten JavaScript business logic, string-built JSON dispatch, arbitrary route graphs, and a separate validator for each client.
- Rationale: privacy and publishing rules must apply outside the TUI. Review reproduced monitor-microphone selections accepted by native configuration and API paths.
- Counterevidence and limits: the frozen 46-case Python corpus covers configuration only. Native web-model tests and a WASM build do not establish browser usability. GPUI is a requirement, not an implemented client.
- Sources: S05, S06 implementation table and review, S07 accepted structures, S09 review regressions. Related risk: R03.

## D03: Phase owns authority, output, and remote cleanup obligations

- Date: 2026-11-13, repaired in `0fbad46`. Adopted historical implementation. Confidence: high.
- Goal: prevent unauthorized peer mutation and false claims that a failed session is clean.
- Adopted: phase variants carry the authority and output they require. Check full camera compatibility before remote Prepare. Record a remote owner immediately before the compatible Prepare attempt. Retain the obligation after a lost response or failed Stop until release is confirmed.
- Rejected: tunnel existence as remote cleanup authority, a permanent independent owner string, parallel optional lifecycle flags, and transition to Idle after an unconfirmed remote Stop.
- Rationale: independent actual-binary review observed an owner-bearing Stop after an incompatible handshake. A response can also be lost after remote allocation, so only recording successful responses would leak cleanup obligations.
- Counterevidence and limits: wholesale separate capture and production phase machines were rejected for this repair because they duplicated transitions. Role-specific runtime capability separation remains incomplete. Serialized effects with individual deadlines do not guarantee immediate aggregate cancellation.
- Sources: S06 review items 2–4, S07 remote ownership repair sketch, S09 red and green ownership tests. Related risks: R02, R04.

## D04: Process groups are cleanup tools, not crash containment

- Date: 2026-11-13. Adopted historical implementation. Confidence: high.
- Goal: release workers, sockets, and audio resources without losing descendant identity.
- Adopted: finite typed worker collection, unreaped leader identity through TERM and KILL, bounded helper I/O, and an outer service cgroup. Production audio uses an owned private Pulse server with only a null sink.
- Rejected: stopping only the leader, a separate string cleanup list, or persistent modules loaded into a shared audio daemon.
- Rationale: descendants can outlive a leader, reaped PIDs can be reused, and shared-daemon modules survive controller death.
- Counterevidence and limits: process groups cannot contain `setsid` escape or clean themselves after controller SIGKILL. The private-audio daemon test established ordinary teardown, not forced-controller-death behavior. Kernel I/O and remote SSH cleanup remain dependency limits.
- Sources: S06 worker/audio rows and review holds, S07 accepted ownership structures, S08 cleanup limits. Related risk: R02.

## D05: Fail closed after Python retirement

- Date: 2026-11-06 initial plan, superseded 2026-11-13. Adopted current historical policy. Confidence: high.
- Goal: remove Python runtime ownership without quietly enabling an unfinished Rust replacement.
- Earlier choice: retain Python as authoritative until parity. Sources: S03 and S04.
- Superseding choice: the operator explicitly required Python source and service delegation removal. A false Rust-candidate flag selects a migration gate, not Python and not Rust. Rollback uses the earlier source revision or an existing generation with separate approval.
- Rejected: hidden Python fallback and implicit Rust selection after removing Python.
- Counterevidence and limits: retirement did not complete parity. Historical publication approved disabled partial scope only. Earlier instructions to keep Python cannot be used to infer that a fallback remains installed.
- Sources: S05, S06 rollback and completion gates, S07 fail-closed decision, S10 migration and review rows. Related risk: R01.

## D06: Media, security, and performance require separate evidence

- Date: 2026-11-05 through 2026-11-13. Adopted acceptance policy. Confidence: high.
- Goal: avoid calling a working process, socket, or synthetic file an accepted stream.
- Adopted: validate selected audio provenance, track types and advancing counters, explicit publication selection, bounded operations, private credentials, and verified listener isolation. Preserve local gameplay audio rather than rerouting it through production.
- Rejected: any sink monitor as game audio, raw cross-host timestamp subtraction, hardware encoding as proof of zero impact, or authentication alone as proof of OBS network isolation.
- Rationale: mixed monitors can include unapproved microphone or speech audio. Clock offsets invalidate naive latency arithmetic. DMA-BUF avoids some copies, not conversion, fences, or memory traffic.
- Counterevidence and limits: incoming byte progress does not prove fresh decoded pixels or bounded frame age. Historical proposed performance thresholds were not measured results. Stock OBS remains conditional on effective ingress tests, not package availability.
- Sources: S02 sections 9–12, S03 stock OBS contract, S04 shared resources, S06 completion gates, S07 limits. Related risks: R01, R04, R05.

## D07: Extract a standalone product without host identity assumptions

- Date: 2026-11-13. Current operator requirement, implementation verification pending. Confidence: high for requirement only.
- Goal: standalone OSS SplitCast with all new source outside the source monorepo.
- Adopted: capture host and production host terminology, repository-local knowledge, stable source IDs, and preservation of original evidence in its source repository.
- Rejected: personal machine aliases, personal absolute paths, and treating extraction as a new successful test run.
- Rationale: source deployment configuration and historical environment details are not portable product defaults.
- Counterevidence and limits: historical Nix closure success does not establish that extracted packages, modules, or commands build independently. No fresh standalone tests are claimed here.
- Source: S00. Related evidence: E06. Related risk: R06.

### D07 correction, 2026-11-14

Fresh E07–E10 now replace the extraction-pass checkpoint's pending local verification. All 124 transferred source files retain original object/content identities. Current code, packages, modules, tests, research and memory live here. The consumer retains only flake wiring and deployment settings. Ignored source-checkout residue is preserved privately under `artifacts/`, not included in publication. No statement about live hardware or production readiness is upgraded.

## D08: Module identity must survive multiple consumer role bridges

- Date: 2026-11-14. Adopted extraction repair.
- Goal: import the same standalone module through independent consumer role switches without defining the same option twice.
- Observation that rejected the first shape: actual consumer evaluation reported `services.splitcast.settings` already declared after both anonymous Home Manager imports were loaded. Direct single-import module tests had not caught it.
- Adopted: a version-specific canonical module key for the curried Home Manager export and a canonical file path for the NixOS export. The consumer's generic role gate evaluator is retained in `nix/consumer-tests.nix`.
- Rejected: deleting the inactive role's option declarations, global keys that silently merge different revisions, or preserving a second local implementation in the consumer.
- Evidence: S14, E09. Full consumer checks have an independent baseline Firefox derivation failure, so targeted gate success is not relabeled as full consumer success.

## D09: Publish an honest experimental source repository, not an acceptance claim

- Date: 2026-11-14. Current publication policy.
- Goal: make the Rust work reviewable and usable by others without transferring personal deployment assumptions or false completion claims.
- Adopted: MIT for original code, explicit upstream GPL notices/license text for the OBS patch and historical copy, conservative Rust 1.95 support, `publish = false` for crates, a normative agent entrypoint, and separate semantic/operational/evidence records.
- Rejected: treating compiler or advisory success as a soundness proof, a stable reusable API promise, or approval to enable runtime capture. F02/F03 in S15 remain runtime blockers.
- Evidence: `THIRD_PARTY.md`, `docs/publication.md`, S14, S15. The human owner still confirms documentation/policy publication rights and creates/pushes the GitHub repository. Until then the consumer uses a pinned local Git bootstrap.

## D10: Prove virtual software behavior independently of hardware acceptance

- Date: 2026-11-14. Current operator requirement and local implementation.
- Goal: connect successful tests to observable control, transport, state-transition, and recovery guarantees without making physical performance a prerequisite for development.
- Adopted: two actual local agent processes with isolated state and stand-in external workers; generated TCP transfer with delay, disconnect, full decode, and resource-release assertions; one absolute HTTP write deadline; atomic queue-expiry and execution transitions; explicit unknown outcomes after transmission or execution starts; owned listener and connection joins; per-instance cancellation with the binary owning signals.
- Rejected: treating increasing fake media counters as decoded frames, using a new operation registry or durable retry protocol for this repair, and describing mandatory joins as a universal hard shutdown bound.
- Observation that changed the implementation: the slow-reader regression renewed a 200 ms budget for 2.003574555 seconds. Independent review also found unrecognized HTTP statuses losing the unknown distinction and Cargo selecting zero tests while the verifier reported a passing opt-in check. The new tests and verifier gate preserve these lessons in executable checks.
- Remaining software work: ordinary Stop still queues behind Prepare; lease maintenance is serial; actual agent RTMP and RTSP media integration, congestion recovery, durable outcomes, and broader crash recovery remain unverified. These are software gaps, not reasons to wait for physical hardware.
- Independent physical gates: Direct Scanout, GPU contention, capture privileges and source provenance, PipeWire and physical audio, real capture and OBS decode, synchronization, gameplay impact, deployment network policy, and cgroup forced-death containment.
- Evidence: S16 and [virtual system acceptance](../virtual-acceptance.md). No deployment gate or autostart target is enabled. Historical S14 results retain their original artifact identity.

## D11: Establish reproducible delivery before expanding production claims

- Date: 2026-11-14 JST. Operator request for memory maintenance, a Production Ready plan, and next-session GitHub Actions CI and Nix caching.
- Adopted planning boundary: this turn maintains documentation and a cold-start handoff. The next session implements CI and the approved cache. No current workflow run, cache upload, credential configuration, commit, push, or deployment is inferred from the request to prepare a handoff.
- Source observation: HEAD remains `aa86fe0` with the repaired source and new tests uncommitted. A fresh HEAD checkout would omit them. No workflow or project-declared cache endpoint/key was found in the inspected tree.
- Recommended sequence: commit the reviewed source deliberately, run clean hosted validation and standalone Nix builds, publish only approved output closures from a successfully validated trusted revision, then prove substitution without fallback builds on a fresh consumer. An Actions acceleration cache alone does not satisfy distribution.
- Service recommendation: use an approved Cachix account initially unless the operator prefers an existing Attic or other cache. The account, destination, public/private policy, public signing key, and write credential are not configured or authorized by this recommendation.
- Rejected: secret-bearing PR execution, untrusted cache writes, caching all flake outputs by default, treating a warm store as consumption evidence, treating cached binaries as production acceptance, or waiting for physical performance before continuing unrelated software work.
- Product boundary: a supported release needs declared features and environments. Recording-first scope is a proposal, not an operator decision. Unknown outcomes need honest recovery semantics; a general durable operation registry is not automatically required. P0 through P4 in the readiness document remain software work, while P5 requires separate target-environment authorization.
- Evidence: S17, [the handoff](../handoff/ci-nix-cache.md), [readiness gates](../production-readiness.md), and [working-state capsule](session-state.md).

## D12: Deliver complete Nix identities through signed GitHub Releases

- Date: 2026-11-14 JST. Operator-selected replacement for D11's proposed external-cache service.
- Goal: distribute prebuilt SplitCast without another account while preserving the existing wrapped native/WASM/OBS package, source-build path and disabled deployment gates.
- Adopted: a full flat narinfo index, official signed NAR forwarding, three signed custom NARs with sources/notices, trusted same-run unprivileged build promotion, separate signing and fresh no-build consumer, and unique per-attempt prerelease tags.
- Rejected: a raw executable-only replacement that drops runtime dependencies; mirroring every third-party NAR; force-moving tags; automatic main rewrites; promoting PR artifacts; and accepting a warm store as cache proof.
- Contrary observations: file-cache bookkeeping names differ across Nix versions, so normalization removes only empty directories structurally, never contents. Drafts are not visible through the public by-tag endpoint, so the publisher retains the ID from creation. Both failures and their original attempts remain recorded.
- Authorization: the owner approved the actual native/web/patched-OBS scope with corresponding sources after GPL/FDK-AAC disclosure, key setup, normal merge and the verification prerelease. This is not a legal/patent audit or deployment authorization.
- Evidence: S18/E14, successful run `43561723796` at `d2205d0`, [receipt](../evidence/ci-nix-cache-20261114-receipt.json), and [download procedure](../binary-cache.md). Next work is P1, not repeating publication or implicitly rotating the configured key.
