---
type: Decision Register
title: decisions
description: D01–D20 selected and rejected designs, falsifications and dated supersessions.
status: stable
sources:
  - { id: primary, resource: ../../packages/splitcast-rs/src/agent.rs }
---
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

Scope clarification: [D15](#d15-recenter-the-poc-on-gameplay-offload-and-independent-audio-return)
retains the consistency rule. [D20](#d20-speech-admission-and-chat-replay-belong-to-the-prepared-session)
records the superseding native browser and speech contracts. The [#20 package comparison](proposed-revisions.md#chat-or-preview-stalls-after-focus-transfer) explains why the proposed desktop client uses Slint after measuring GPUI.

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

See [D15](#d15-recenter-the-poc-on-gameplay-offload-and-independent-audio-return)
for the renewed three-budget mission; no prior synthetic envelope becomes a
gameplay or returned-audio latency target.

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
- Evidence: S17, [the handoff](https://github.com/tenzyu/splitcast/blob/archive/pre-dreaming-20261117/docs/handoff/ci-nix-cache.md), [readiness gates](../production-readiness.md), and [working-state capsule](session-state.md).

## D12: Deliver complete Nix identities through signed GitHub Releases

- Date: 2026-11-14 JST. Operator-selected replacement for D11's proposed external-cache service.
- Goal: distribute prebuilt SplitCast without another account while preserving the existing wrapped native/WASM/OBS package, source-build path and disabled deployment gates.
- Adopted: a full flat narinfo index, official signed NAR forwarding, three signed custom NARs with sources/notices, trusted same-run unprivileged build promotion, separate signing and fresh no-build consumer, and unique per-attempt prerelease tags.
- Rejected: a raw executable-only replacement that drops runtime dependencies; mirroring every third-party NAR; force-moving tags; automatic main rewrites; promoting PR artifacts; and accepting a warm store as cache proof.
- Contrary observations: file-cache bookkeeping names differ across Nix versions, so normalization removes only empty directories structurally, never contents. Drafts are not visible through the public by-tag endpoint, so the publisher retains the ID from creation. Both failures and their original attempts remain recorded.
- Authorization: the owner approved the actual native/web/patched-OBS scope with corresponding sources after GPL/FDK-AAC disclosure, key setup, normal merge and the verification prerelease. This is not a legal/patent audit or deployment authorization.
- Evidence: S18/E14, successful run `43561723796` at `d2205d0`, [receipt](../evidence/ci-nix-cache-20261114-receipt.json), and [download procedure](../binary-cache.md). Next work is P1, not repeating publication or implicitly rotating the configured key.

## D13: Separate Stop release authority and remote dispatch from activation

- Date: 2026-11-14 JST. PR #2 candidate `review/urgent-control`, implementation `5125799`, not main or deployment acceptance.
- Adopted: one effect owner, independent authenticated Stop admission, generation fences, owner-bound release after lease expiry, and remote Prepare bookkeeping at the first positive socket write.
- Rejected: applying renewal expiry to cleanup retries, recording unsent remote effects, treating 202 or plain Idle as release, and silently replaying uncertain Start or Prepare.
- Accepted limits: observation is ephemeral without per-intent IDs, later authenticated Prepare is not cross-host replay-fenced, and finite parser slots can exclude HTTP control. Clients must coordinate observation and fresh authorization. No durable registry or executor rewrite was added.
- Evidence: [S19/E15](evidence.md) and [the review follow-up](../evidence/urgent-control-review-20261114.md). The original D10/S16 statements retain their dated source scope; this candidate supersedes their ordinary Stop and inbound lease-progress limitations only. Outbound health work, media-path integration, browser ownership, kernel stalls, and deployment containment remain separate.

### D13 correction, 2026-11-15 JST

S20 inspected actual main `2a774d9`, whose history contains PR #2 merge `6349fd6`. D13's unmerged-candidate description remains true for its earlier S19 checkpoint, not current main. No deployed-state claim changes. S20's media changes do not replace or reimplement that control work.

## D14: Accept decoded production media independently of liveness

- Date: 2026-11-15 JST. Initial implementation `0accf63`, corrected acceptance at `42a64e4`; sources S20/E16 and [the media receipt](../evidence/media-plane-20261115.md).
- Goal: distinguish correct generated media traversing the actual production services from living processes and increasing counters. Keep physical capture and deployment separate.
- Adopted: a single opt-in Python stdlib profile around the real production Agent, real MediaMTX, private Pulse/RNNoise, and real patched OBS. Synthetic publishers replace only hardware ingress. A typed optional OBS video mode selects software encoding and decoding without changing legacy hardware defaults. The namespace launcher hides physical media devices and isolates network, PID, and writable-state ownership.
- Rejected: a detached FFmpeg recording, fabricated OBS status, profile-rewriting wrappers, independent encoder/decoder knobs, and more elaborate Rust FIFO/thread orchestration for this experiment. A naive delayed RTMP relay was not accepted because it stalled control exchanges rather than proving delayed advancing media. The adopted bounded source backlog is explicitly not network impairment.
- Disproved assumptions: software rendering alone prevented device enumeration; a WebSocket connection meant the scene was loaded; RNNoise could start before RTSP publication; active output implied usable bytes or pixels; StopRecord acknowledgement proved recording finalization; total marker span ruled out a frozen tail. The receipt separates direct observations, fixture failures, and the first-health counter cause that remains unisolated.
- Product changes: wait for the initialized scene, track readiness before RNNoise, source geometry and actual bytes before Running, and inactive output before teardown. Propagate unconfirmed OBS finalization while continuing cleanup. Inherit worker stderr so the original service error remains observable.
- Observed: the final profile decodes real local outputs, identifies advancing source epochs and both audio identities, measures sampled local scene age and game A/V marker offset, rejects frozen/backlogged content despite byte progress, and verifies faults, release, and explicit fresh recovery. Source, build, executable, service, fixture, and output identities remain separate.
- Limit: Agent health still admits stale advancing content. The independent test detects it; live enforcement is not implemented. No capture-Agent/SSH, real network delay, microphone alignment, default-VAD speech quality, physical gameplay, or deployment acceptance follows. P1 and R04 are narrowed, not closed.
- Correction links: D06's counter limitation and D10/S16's original missing-integration statement remain preserved. S20 supersedes only the production-only generated-media gap, not their broader timing, hardware, or containment risks. Source records and earlier failed runs are not erased.

### AV01 correction, 2026-11-15 JST

D14/S20's initial `0accf63` phase-fit timing acceptance was falsified: an 800 ms delay could fit inside its claimed 500 ms budget. Intermediate repeated codes, hidden PCM timestamp gaps and incorrect opening identity were also disproved. `42a64e4` adopts unique source-second audio identities, decoded sample/PTS continuity, sampled transition bounds with uncertainty, and bounded stream endpoints. Partial EOF markers outside joint observation are reported as unmeasured. Eleven pure tests, six real cases and three successfully generated recording mutations pass in E16's corrected receipt. The claim is sampled game A/V timing only; microphone alignment and continuous synchronization remain unknown. Human review is pending, not approved.

### D14 scope and diagnostic correction, 2026-11-17

S21's review identifies additional oracle gaps; see D16. The earlier passes keep
their source scope, not a claim that those gaps were checked. P2-04 restores
worker stderr to null rather than unconditionally inheriting it into production
logs. Original service failures informed S20's repairs; no actual secret leakage
was established. The fixture's synthetic worker diagnostics are explicitly
redirected inside its private test directories. Production diagnostic design
remains separate. D14 selected an acceptance fixture, not the optimal product
architecture; D15 makes that distinction explicit.

## D15: Recenter the PoC on gameplay offload and independent audio return

- Date: 2026-11-17. Current user requirement, S21. Not implementation acceptance.
- Goal: protect capture-host osu! Input/Audio/Update/Draw/display while moving
  production work away; independently return production-computed speech/audio
  to capture-host playback. Minimize shared CPU/GPU/memory/thermal/power and
  scheduling interference; streaming failure must remain downstream of gameplay.
- Adopted knowledge correction: [the mission](poc-mission.md) and
  [environment-gap handoff](environment-gap.md) now lead the next session.
  Reuse software and keep SplitCast thin. Distinguish infrequent control from
  the media hot path; prove a specific bottleneck before duplicating infrastructure.
- Supersedes: treating TTS as dispensable product scope because the current
  candidate refuses it, or treating a recording-first release recommendation
  (D11) as the user's adopted mission. D02's historical validation/view policy,
  D06's evidence separation and D14's experimental route remain dated decisions,
  not proof of an optimal transport or complete PoC.
- Observation: `agent.rs::prepare` refuses TTS/EventSub/remote mixing;
  `media.rs::tunnel` forwards control/RTMP/RTSP, not returned-audio playback.
  GSR and OBS remain dependencies. S06 already distinguished synthesis from
  delivery; E16 bypasses the capture side entirely.
- Open: environment capability gaps; necessary local encoding work; return-path
  isolation/expiry/playback contract; three independent acceptance budgets.
  No TTS-first sequence, chosen protocol, concrete numeric target or new PoC is
  authorized or accepted. No private deployment details belong in public docs.

## D16: Judge cadence, audio coverage and fixture health from observations

- Date: 2026-11-17. S21 review follow-up, implementation/evidence S22.
- Counterexamples: nominal 30 fps and 270 frames can describe a 25%-drop VFR file;
  exact 4,800/9,600-sample PCM lengths lose one complete window; 35 missing mic
  windows of 118 still pass 70% presence. Receipt-only feeder errors do not
  establish healthy ingress. The original Prepare reset has no established cause.
- Adopted: decoded PTS density/local counts/gaps over a declared steady prefix;
  every complete 100 ms PCM window, partial tail explicitly unclassified;
  consecutive-absence checks; unexpected source errors/exits fail, with explicit
  fault/Stop boundaries; bounded reset reproduction with private per-agent traces.
- Verification and residual limits: [the review follow-up](../evidence/pr3-review-20261117.md)
  records controls, source identities and commands. Synthetic VAD-zero tones are
  not speech-quality or microphone-alignment proof. No live freshness enforcement,
  new transport, broader hardening or speculative reset fix is introduced.

## D17: Verify finite speech playback before wiring a delivery service

- Date: 2026-11-17. S24 implementation decision, not complete PoC acceptance.
- The operator requires game video, game-only audio, microphone offload and real
  Twitch-triggered returned TTS. Camera is excluded. Baseline precedes proposed
  performance limits. Temporary trials have permission distinct from deployment.
- Adopted: one explicit fixture playback operation reuses native `pw-play` and
  the existing process owner. Shared strict PCM validation happens before spawn;
  raw stdin avoids a second WAV cache and playback-file deletion state. Player
  completion is not physical listening. Ordinary Agent refusals remain intact.
- Rejected: a new Agent WAV endpoint/claim service, a second synthesizer or media
  stack, removing refusals to represent unfinished delivery, and calling local
  test text a real Twitch event. Future transport APIs are not added as stubs.
- Authority boundary: read-only capture SSH does not authorize remote worker
  execution. Missing capture helper rights do not authorize provisioning. A new
  Twitch auth/subscription operation remains separately gated. Safe independent
  work continues while those operations are blocked.
- Evidence and limits: [S24 checkpoint](../evidence/poc-20261117.md). OFF Direct
  Scanout samples are not controlled all-frame baseline data. Generated hardware
  codec execution and real local engine synthesis are separate from physical
  capture, microphone routing, event ingestion, returned listening and integration.

## D18: Separate granted Twitch operations from absent account inputs

- Date: 2026-11-17, S25. EventSub WSS, configured-channel chat v1 subscription,
  necessary Helix calls and existing suitable OAuth use are now approved. Do not
  re-request those grants. New OAuth or scope expansion remains forbidden.
- Observed: production's configured OAuth file is absent and no channel selector
  exists. Capture-side OBS has a Twitch stream key but no selected OAuth/account
  metadata in the inspected profile and global configuration. Do not treat a
  stream key as chat OAuth or infer a channel from repository ownership.
- Adopted: stop only the missing-input operation, request the existing account
  path or receiver output, and avoid implementing a duplicate client against an
  unknown source. Keep mock, local synthesis and real event evidence distinct.
- Capture discovery corrects the earlier coarse dependency hypothesis. A cached
  FFmpeg exists but lacks the Pulse demuxer. GSR and the privileged KMS helper
  are absent. Existing native audio and portal tools are candidates, not verified
  replacements or permission to run workers through read-only SSH.
- Source/evidence: [S25 checkpoint](../evidence/poc-convergence-20261117.md).
  Existing fixture work is locally committed, not promoted to complete PoC.

## D19: One owned watchdog governs timed output

The trial represented at review head `0823165` used Agent deadlines of 12
seconds for public and 20 seconds for private output, plus independent
OBS timeouts at 18 and 28 seconds respectively. The two limits started at
different points. The findings establish a deadline conflict and a separate
requirement to investigate the observed OBS crash.

The production lifecycle accepts an optional bounded duration at activation.
Public Start requires per-command confirmation, duration and the current
prepared-session Start ID. A delayed local request can carry an ID that no
longer matches the state or generation. The same Stop admission and
generation fence enforce expiry. Capture renews its owner lease while
healthy. Production stays idle after recovery until a new Start is admitted.

OBS output inactivity and the process exit status are separate evidence.
The controller asks the directly owned OBS process to close after the
output Stop, then limits cleanup using the process group. An unreaped
leader retains group identity. The enclosing service remains the
containment boundary. Controller-managed Stop IDs correlate current
observations while owner identity establishes remote release.
[Original review S31](https://github.com/tenzyu/splitcast/pull/16#pullrequestreview-5950165310) · [Operations](../operations.md#stop-and-rollback)

## D20: Speech admission and chat replay belong to the prepared session

The native Agent serves Web assets and owns the single authenticated
command contract. This supersedes the browser transport and implementation
choices recorded in D02. For Speak, the client retrieves the current
`next_id` and submits the text once with that ID. Admission state belongs
to the Agent. Stale or consumed IDs fail before a second enqueue.

Production validates EventSub subscription identity and holds one
cancellation-aware reconnect worker per prepared session. That session
keeps bounded replay state across reconnects. The retention contract covers
the 4,096 most recent distinct chat message IDs for up to ten minutes.
A queue drop consumes the same ID. Expiry, eviction, or a new session
allows an older ID to be seen again. This permits long sessions with memory
bounded by message count and lifetime. [Operations](../operations.md#native-poc-startup)
