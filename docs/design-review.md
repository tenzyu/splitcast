# Standalone SplitCast semantic review

Reviewed 2026-11-13 through 2026-11-14. **Review only, not acceptance or deployment authorization.**

Scope is the standalone workspace, native controller, portable protocol, browser client, and extracted Nix integration. The reviewer changed only this document. The extracting session owns implementation, portable-tool changes, and final verification. References below name the inspected source and line ranges, not immutable standalone commit identities. Concurrent edits require a final reference check.

Unqualified native filenames below refer to `packages/splitcast-rs/src/`. `browser.rs` refers to `packages/splitcast-web/src/browser.rs`. All paths are relative to this standalone repository.

The extraction source is `3c8857353581424e5cca517d7ccf0db8de7514d4`. Historical publication `21a4f8828f326af1eaa28c4a0c56fef03874296f`, including repair `0fbad465122f538eda746704e151dff51c7d6102`, is a different evidence identity. Neither proves that this standalone artifact builds or runs. See `docs/memory/evidence.md:9-16,40-47`.

## Decision and severity

The design is suitable for continued work as a disabled experimental candidate, not for a stable library release or runtime cutover. No memory-unsound operation was demonstrated by this review. That statement is not an FFI soundness proof.

- P1 means a broken advertised integration or a lifecycle defect that blocks deployment acceptance.
- P2 means an API, error, or verification weakness that needs a stated experimental restriction and a focused follow-up.
- Documented missing features and unmeasured behavior remain acceptance gates, not newly proven regressions.

F01's NixOS export failure was reproduced and then corrected by the extracting session. The final isolated export-shape probe passes, but consumer evaluation remains unrun here. F02, the lack of interruptible, owned, bounded whole-server shutdown, remains a deployment blocker. F03 requires an explicit accepted-command cancellation contract before users can safely interpret a timeout. F04 through F07 can remain documented restrictions for a `publish = false` candidate, but not implicit promises of a stable validated API.

## CONTRACT

The observable contract is narrower than full SplitCast parity:

1. Configuration validation is offline. It must not capture, create an agent listener, or require credentials. `packages/splitcast-rs/src/main.rs:168-176` returns before the agent path.
2. Agent startup creates a loopback control listener but starts no media output. A separate operator authorization is required even to run that listener. `control.rs:234-237` and `agent.rs:674-694` define this boundary.
3. An omitted output selects recording, never Twitch. `packages/splitcast-protocol/src/lib.rs:176-186` and native CLI `main.rs:68-83` agree. Browser publication requires its separate confirmation, but confirmation does not replace operator permission.
4. Capture preparation checks the production role and Rust-candidate identity, compares routes and transport, then records a remote cleanup obligation before sending Prepare. `agent.rs:124-132,253-265,300-307` preserves the lost-response case.
5. A session reaches Running only after start effects and observations complete. Repeating Start with the same active output succeeds without switching outputs. Stop retains Stopping after incomplete cleanup. See `agent.rs:333-404,451-470`.
6. Receive readiness means expected tracks, positive and increasing byte counts, and a recent local observation. It does not mean fresh decoded frames, synchronized audio, or bounded end-to-end latency. See `media.rs:81-117`.
7. Enabled EventSub, speech delivery, and remote TTS mixing refuse preparation. Live TTS commands report incomplete delivery and playback. See `agent.rs:293-295,588`. The synthesis component is not an implemented end-to-end feature.

The type-level and runtime invariants differ:

- Private `Session.phase` variants bind authority and output to lifecycle states. Idle has neither, and Running carries an output. `lifecycle.rs:26-62,118-122` makes those structural properties explicit.
- `OwnerId` has a private string and a validating constructor. `packages/splitcast-protocol/src/lib.rs:64-77` prevents safe construction of an invalid owner through its public API.
- Worker keys are an enum and `Workers` owns processes without Clone. `workers.rs:6-17,33-47` prevents accidental duplication of process ownership.
- Configuration validity is only a constructor-time runtime check. Public fields can invalidate it afterward. Protocol commands likewise contain unrestricted public strings and JSON values. These are F04, not validated types merely because a parser exists.
- Resource release, remote completion, and lease progress still depend on I/O, scheduling, and cleanup results. A lifecycle enum cannot establish those external facts.

### F01: NixOS module export applied the wrong function, P1, corrected during review

At the initial inspection, `flake.nix:35` exported `import ./nix/modules/nixos.nix { inherit self; }`. Unlike the Home Manager module, `nix/modules/nixos.nix:1-6` is already the module function and requires `config`, `lib`, and `pkgs`. Applying only `self` fails before the NixOS module system can supply those arguments.

The isolated evaluation in VERIFICATION reproduced `function 'anonymous lambda' called without required argument 'config'`, exit 1. The extracting session subsequently changed `flake.nix:35` to the unapplied import. The final probe evaluates the actual flake export as a module function with `config`, `lib`, and `pkgs` arguments, exit 0. This closes the demonstrated import-shape defect, not the full consumer-integration gate. A minimal NixOS consumer evaluation is still needed. No activation is required or authorized.

All product implementation, package expressions, patches, modules, and test drivers belong here. `nix/source.nix:2-18` uses a repository-local fileset, and package dependencies use relative paths. That is favorable source evidence, not a completed closure audit. Historical material under `docs/history/source/` is evidence, not a second runnable implementation or current deployment instruction. The parent session owns the final portable-tool scan and source-monorepo retirement check.

The final source read also confirms that `packages/splitcast-rs/tests/synthetic_ssh.rs:12-88` now requires explicit authorization, source host, identity, and remote FFmpeg configuration. Local FFmpeg and ffprobe resolve through configurable values or PATH. The previously embedded staged tool paths are not an unresolved finding in that inspected file. The reviewer did not execute the synthetic trial or verify its revised teardown.

## OWNERSHIP

The native agent has one mutable owner on the control loop. It owns `Config`, `Session`, `Workers`, monitor snapshots, and the exclusive role lock. The lock file remains alive in `_lock` until Agent destruction. `agent.rs:149-160,169-188` ties this local ownership to a private role directory.

`Workers` owns each `OwnedProcess`. The unreaped process leader pins the group identity until TERM, KILL, and leader reaping complete. `process.rs:26-34,63-130` is stronger than treating an observed leader exit as complete cleanup. A failed `Workers::stop` retains the process for another attempt. `workers.rs:54-74` attempts every owned worker rather than returning after the first failure.

The process-group promise has limits declared in `process.rs:1-8`: descendants can escape the group, controller SIGKILL prevents Drop, and kernel-stuck tasks cannot promise bounded reaping. The enclosing service cgroup is therefore part of containment, not an optional optimization. `Drop` is best effort. It does not establish crash cleanup or remote publication shutdown.

Connections own their `TcpStream`, copied token, request, and response channel. Asset bytes use `Arc<Assets>`, so each connection shares one immutable asset allocation. The Slot guard decrements the connection count even during unwinding. The eight-slot accept limit and bounded request queue are real local resource controls. The missing JoinHandles are F02.

Clone decisions at the inspected sites are different kinds of ownership:

- `cfg.clone()` at `agent.rs:677` duplicates strings, paths, and the normalized JSON document. It separates the agent's mutable selected configuration from serving metadata. Narrowing the retained metadata would avoid the full copy, but no measured allocation problem justifies redesign now.
- `Arc::clone` at `agent.rs:703,707` shares counters and immutable assets. It does not duplicate asset buffers.
- Token Clone at `agent.rs:705` makes a secret-bearing string per connection. Browser token Clone at `browser.rs:159` similarly snapshots authorization for each request. Clearing the stored token does not erase those in-flight copies. No zeroization guarantee exists.
- Authority Clone during lifecycle transitions copies the small owner identity, not a remote lease or a process. JSON Clone in settings and protocol routes is a value snapshot with allocation. It is not shared mutable state.
- Browser `Rc` and DOM Clone operations share browser handles within one thread. They do not clone the page contents.

`Speech` owns at most one synthesis worker and its JoinHandle, pending and ready jobs, and the current playback claim. Cancellation clears the worker's publication permission before its result is collected. Drop requests cancellation and joins. See `tts.rs:253-284,415-475,578-590`. Cached-file removal failures are ignored, and crashes can leave cache files. There is no durable replay or exactly-once delivery guarantee.

### F04: Public configuration bypasses the validated boundary, P2

`config.rs:280-334` exposes every field, including `role`, ports, sink names, paths, timing, and the normalized `document`. Callers can construct a configuration that disagrees with its document or mutate values after `Config::parse` succeeds. `Agent::new` treats every role string other than `capture` as production at `agent.rs:163-168`.

This affects more than formatting. `audio.rs:61` inserts `processed_sink` into a Pulse configuration script without revalidating the field. The parser rejects newlines and invalid sink names, but a safe library caller can bypass that parser. `media.rs:260-261` similarly relies on validated filter fields. This is not a demonstrated network configuration injection through `Config::load`, and it is not proof of memory unsoundness.

The smallest defensible API is a private-field Config with a validated constructor and controlled selection updates. Internal test fixtures do not justify exposing invariant-bearing fields to downstream users. If the internal API stays public temporarily, document that it is unsupported application internals, not a validated domain API.

### F05: FFI proof obligations are incomplete, P2

The inspected FFI uses libc and small UID declarations, not a custom frame ABI. The critical sites are `process.rs:68-95,141-145`, `agent.rs:183-186,680-685`, and the ownership checks in `state.rs:38-44`, `config.rs:255-258`, and `audio.rs:34-37`.

No raw frame buffer, manual Send or Sync implementation, or callback capturing borrowed Rust state was found in these reviewed modules. `waitid` receives a live, aligned `libc::siginfo_t` and requests exited-child information. The union reads require successful `waitid`, a nonzero reported PID, and a matching exit notification. Group signaling relies on the unreaped leader and on callers not reaping it elsewhere. `fcntl` uses borrowed live file descriptors. The signal handler only stores an atomic flag on the explicitly supported native targets.

Those arguments need adjacent `SAFETY` explanations and a documented platform contract. Most unsafe blocks have none. Handwritten `getuid` declarations duplicate libc's ABI definition unnecessarily. Do not call these missing explanations a proven UB finding. Conversely, successful native tests cannot prove all ABI, signal, lifetime, and process-identity obligations. Miri was not run and cannot validate the real Linux subprocess interactions by itself.

## FAILURE / STATE & ERRORS

### F06: Error strings lose recovery and commit information, P2

The code generally returns `io::Result`, but many domain failures collapse to string messages. `control.rs:14-16` classifies malformed framing, invalid credentials, oversized commands, and rejected responses as PermissionDenied. `agent.rs:761-765` maps every command error to HTTP 409 with only an `error` string. `http.rs:18-37` loses network causes, including distinguishable timeout information. `workers.rs:61-74` reports failed worker names without their causes.

This prevents callers from distinguishing invalid input, authorization, ownership conflict, unsupported features, queue pressure, transient network failure, timeout, cancellation, and incomplete cleanup without parsing English. `run_bounded` can also replace the original operation failure with `cleanup?` at `process.rs:251-264`. Both failures matter.

Keep `io::Error` for operating-system operations, but define a small domain error enum with `Error::source` and stable machine-readable response codes at the application boundary. Preserve both the primary failure and cleanup failures. A general error-reporting dependency is not required to make these distinctions.

Commit points and retry behavior are as follows:

- Prepare records its phase before effects. Capture records the remote owner before sending remote Prepare. Failure invokes cleanup and keeps an unconfirmed remote obligation. `agent.rs:289-307,406-449` makes a lost response different from no request having been sent.
- Start can launch workers and start an OBS output before its confirmation arrives. A timeout is therefore not evidence of no output. The failure path attempts Stop and kills owned workers. Residual remote failure depends on a production lease and its control loop making progress.
- Stop is retryable while the process remains alive. The same remote owner survives an unconfirmed Stop, and a failed child cleanup remains owned. Idle is reached only after cleanup succeeds. This is an in-memory recovery contract, not a durable transaction across controller crashes.
- Settings use write, file sync, rename, and directory sync in `state.rs:76-80`. Rename is the visible replacement commit. A later directory-sync failure can return an error after the new file is visible. `agent.rs:638-640` updates memory only on success, so disk and memory can disagree after that failure. Read-back or an explicit indeterminate-commit error is needed for stronger atomicity claims. Blind retry cannot establish that the first attempt did nothing.
- OBS configuration is a sequence of acknowledged effects, not a transaction. Private configuration and the owned OBS process limit the damage, but Stop does not roll back every generated file or OBS setting.
- `retry_limit` is parsed at `config.rs:438`, while live diagnostics report zero retries at `agent.rs:568`. There is no implemented general reconnect budget. The speech component separately retries synthesis up to three attempts without an error-category policy or backoff, `tts.rs:435-449,457-469`. Its 1,024-entry in-memory ID history is bounded deduplication, not persistent exactly-once playback.

No new automatic retry or publication is recommended as part of extraction. Explicit refusal is safer than retrying an effect whose completion is unknown.

## ASYNC / ASYNC & SHUTDOWN

### F02: Whole-server cancellation and joining are not bounded, P1

The server is synchronous even though connections use threads. Signal handling only sets `SHUTDOWN`; the flag is read at the outer loop in `agent.rs:694`. `wait_ready` at `235-246` never checks it. Preparation, OBS configuration, discovery, health checks, and cleanup run synchronously on the same owner that accepts commands and expires leases.

Consequently, an accepted Prepare or discovery operation can delay emergency Stop, lease expiry, and SIGTERM handling until its current work returns. Individual five-second I/O deadlines do not bound the sum of sequential operations. A readiness check can also finish after the nominal wait deadline because the closure has its own timeout. The 20-second service stop timeout in `nix/modules/home-manager.nix:103` is an external forced-kill fallback, not a demonstrated graceful-stop bound.

At `agent.rs:708-743`, `thread::spawn` discards every JoinHandle. Connection threads can wait 180 seconds for a response. `serve` returns after `agent.stop(None)` without closing and joining them, `agent.rs:770-772`. A process exiting terminates such threads, but a caller of the public library function can retain detached threads and accepted sockets after serve returns. Ordinary binary exit tests do not establish library lifecycle cleanup.

Response writes also lack a total deadline. `control.rs:186-197` sets a socket write timeout once and calls `write_all`. That timeout applies to individual blocking writes, not the entire response. Large assets can consume a connection slot much longer than five seconds when a reader makes intermittent progress. The finite asset cap bounds bytes, not an acceptable wall-clock service bound. No slow-reader timing measurement was run here.

The minimal fix needs one shutdown owner and explicit stages: stop accepting, reject queued work, cancel in-flight preparation, close sockets, stop owned workers, and join tracked connection threads under one documented deadline. Long effects need cancellation checks between calls and deadlines derived from the remaining operation budget. The accepted limitation for process-group escape and kernel-stuck tasks still requires the outer cgroup. Introducing Tokio alone would solve none of these obligations.

### F03: A timed-out or abandoned request can still commit, P1

After `try_send` succeeds at `agent.rs:733-741`, the connection waits for an answer. A timeout drops its answer receiver, but does not remove the queued command. The owner executes it at `agent.rs:748-767`, and discards a failed answer send. A disconnected client likewise has no cancellation path back to the operation. A client can therefore observe failure and later have an accepted Start or settings change execute.

This does not bypass authentication, and it is a common RPC ambiguity. It must still be explicit because Start can publish. Define whether acceptance commits the operation regardless of caller lifetime. If so, return an operation ID, expose its status, and make retries refer to that ID. If cancellation before execution is intended, carry that state into the queue and check it before effects. After an external effect may have committed, report an unknown outcome rather than claim cancellation undid it. Do not retry Start on transport failure automatically.

Backpressure is presently rejection, not waiting: at most eight connection workers and a queue of eight, with HTTP 503 on queue refusal. Each connection owns at most one queued request. `agent.rs:698-702,733-737` bounds native fan-out. The 20 ms loop sleep and serialized effects do not establish fairness or a response-time bound under expensive work. A slow operation also delays heartbeat and lease handling.

### F07: Browser requests have no cancellation or concurrency owner, P2

`packages/splitcast-web/src/browser.rs:58-86` awaits fetch and then the response text without an AbortController or application timeout. Every click launches another `spawn_local` future at `147-182`. No handle tracks completion, disables duplicate commands, or orders status updates. Older responses can overwrite newer observations, and repeated clicks can create more waiting futures despite the native queue limit.

Dropping a Rust future is not an explicit browser fetch abort. There is no page-level cancellation path or stale-response generation check. Token replacement does not revoke authorization already copied into pending futures. This is not proof that unload leaves requests running in every browser; that behavior was not tested. The server-side accepted-command ambiguity in F03 remains even if fetch is aborted.

The page uses `text_content`, not HTML interpolation, and requests SameOrigin mode. Those are useful boundaries. `Closure::forget` at `123,185` keeps listeners for the page lifetime. That is tolerable for a one-shot page, but not a reusable mount/unmount API. Browser acceptance needs command serialization or explicit concurrency policy, an abort and timeout owner, and stale-response handling through the real DOM and API, not just native JSON tests.

## PUBLIC API

All three product crates currently have `publish = false`. Retain that initial policy. Publishing source as OSS does not require promising stable Rust library APIs. `packages/splitcast-rs/src/lib.rs:1-18` currently exports nearly every implementation module, including process, state, audio, HTTP, and server functions. Those exports expose operational effects and future compatibility obligations far beyond the intended clients.

The minimum policy is an application-first workspace with an experimental portable command crate. Keep helpers private or `pub(crate)` where possible. Document the deliberately supported command behavior separately from internal Rust symbols. A future stable protocol crate needs validated payload types and a compatibility decision before publication.

Specific downstream commitments need attention:

- `Command` is exhaustive and publicly constructible at `packages/splitcast-protocol/src/lib.rs:37-56`. Adding variants breaks downstream exhaustive matches. `Routes { settings: Value }`, raw text, and raw ID strings permit values that `Invocation::parse` rejects. `to_body` serializes them without validation at `231-239`. A successful constructor or serialization is not currently proof of a valid wire command.
- `Invocation::from_body` caps a body at 16 KiB and rejects unknown top-level and argument fields. `Invocation::parse` is a separate public entry point without that byte cap. `serde_json::Value` parsing also does not preserve duplicate object-key evidence for subsequent validation. Do not advertise rejection of every ambiguous JSON representation without a duplicate-key policy and tests.
- The request body has no version field. Status and the transport description carry version 1, but `require_rust_production` checks only that version, role, and the `rust-candidate` label. It is a compatibility check, not a cryptographic build identity or capability negotiation. Response schemas and error codes are untyped JSON. Specify unknown-field behavior, size limits, defaulting, supported command subsets, and version migration before claiming mixed-version interoperability.
- `splitcast-web/src/lib.rs:4-7` exposes `command(name, args)` but panics on invalid caller input through `expect`. The invariant belongs to current view call sites, not all safe public callers. Make that helper internal or return a Result.
- `agent::serve` installs process-global signal handlers without restoring them. `SHUTDOWN` is a static flag that is not reset. A second same-process call after a handled shutdown does not behave like a fresh server. `agent.rs:17-20,680-694` is an application entry-point design, not a reusable server API.
- There are no public custom extension traits to justify repository or service abstractions. The existing private `wait_ready<F: FnMut() -> bool>` and I/O-generic helpers use static dispatch for local callbacks. Keep them concrete and local. Standard Read and Write adapters for `DeadlineStream` are appropriate. No public `dyn Trait` layer or new generic framework is needed.
- Public Send and Sync properties, panic behavior, defaults, feature sets, CLI flags, configuration schema, and wire responses all become compatibility obligations if these crates are published. `publish = false` limits accidental registry release; it does not make promises in user documentation harmless.

`Cargo.toml:5-8` and `rust-toolchain.toml:1-5` declare Rust 1.95 and pin 1.95.0. Treat 1.95 as a conservative supported floor matching the chosen toolchain, not an empirically discovered minimum compiler version. No lower-toolchain matrix was run. The workspace uses edition 2021 and explicit resolver 2. The process module restricts native code to Linux x86_64 and aarch64, `process.rs:10-14`, while the flake exposes only x86_64-linux, `flake.nix:6`. Broad Linux package metadata is not evidence for other architectures. WASM belongs to the web and protocol crates, not the native controller.

## PERFORMANCE / TRADEOFFS

No performance benchmark ran in this review. There is no fresh standalone measured baseline, achieved target, speedup, or regression percentage to report.

The historical evidence register reports a 600.022-second synthetic recording with 9,000 video packets and 28,126 audio packets, `docs/memory/evidence.md:46`. That is documentary evidence for a separate software-generated SSH transport trial, not native agent throughput, production OBS performance, physical audio latency, or gameplay non-regression. Historical driver-quantum observations likewise do not establish a physical latency gain.

Before a performance acceptance decision, define the workload and measure a paired game-only, capture-enabled, then game-only comparison under matched thermal, power, display, and network conditions. Relevant metrics include game frame and input tails, audio underruns, capture and decoded-frame age, clock correspondence, media queue depth, CPU and GPU use, memory, and total shutdown time. A native command test or a healthy byte counter cannot substitute for those measurements.

The target is supported non-regression of gameplay with capture stopped or degraded first under contention. There is no justified numeric equivalence margin yet. Set that margin from a controlled baseline before comparing candidates, not after seeing results. Reject cutover when the margin cannot be supported, frame age or queues grow, privacy selection changes, shutdown exceeds the agreed bound, or local gameplay audio depends on production. This preserves the historical A/B/A and uncertainty policy in `docs/history/source/docs/graphics-streaming/ADR-2026-11-07-performance-first.md:26-28` without treating that proposal as a completed measurement.

The current subprocess architecture trades direct control of every media buffer for process isolation and existing native codecs. TCP and SSH favor deployability but can retain stale media under head-of-line blocking. Hardware encoding does not prove zero GPU or memory contention. Private audio isolates production routing but adds a process and startup sequence. These are reasonable candidate choices, not established optimums. Do not add pooling, zero-copy abstractions, unsafe capture, or clone-avoidance changes without evidence of the limiting resource.

## VERIFICATION

The reviewer read `RUST_AGENT_ROOT.md`, all relevant correctness, ownership, errors, async, FFI, public API, performance, and verification sections of `RUST_ENGINEERING_AGENTS.md`, `MEMORY.md`, and the available memory pages. At the initial read, `docs/memory/INDEX.md`, `docs/operations.md`, and this review file did not exist. The final source read included the newly created memory index. The operations guide remained unavailable at that read. No operational tests were attempted, and later creation of that guide does not mean its final contents were reviewed here.

Exact shell commands run by this reviewer from the standalone root:

| Command | Result | What it proves |
| --- | --- | --- |
| `command -v nix cargo rustc` | Exit 1. Nix was found; Cargo and rustc were not found on the current PATH. | Direct Cargo commands need the declared development environment. It does not prove Rust is unavailable through Nix. |
| `nix eval --impure --offline --expr '(import ./nix/modules/nixos.nix { self = {}; }).options'` | Exit 1. `function 'anonymous lambda' called without required argument 'config'`. | Reproduces F01's extra module-function application without starting services or evaluating a host configuration. |
| `nix eval --impure --offline --json --expr 'builtins.functionArgs (((import ./flake.nix).outputs { self = {}; nixpkgs = {}; }).nixosModules.default)'` | Exit 0. `{"config":false,"lib":false,"pkgs":false}`. | After the parent fix, the actual export is the unapplied module function. This is not a full NixOS consumer evaluation. |

Source reads and searches support the static findings above. They do not reproduce a shutdown delay, stale browser response, ABI failure, or partial disk write. Existing test definitions were inspected, not rerun. In particular, `packages/splitcast-rs/tests/control_integration.rs:890-932` covers an incomplete HTTP client and binary SIGTERM while idle. It does not cover SIGTERM during preparation, joining library threads, cancellation after queue acceptance, or response-write deadlines.

The reviewer did **not** run:

- `nix develop -c cargo run -p splitcast-xtask -- verify`, including fmt, workspace check, warning-denied Clippy, tests, doctests, feature checks, WASM check, and dependency-tree inspection. The extracting session owns the final run and exact results.
- Nix package or WASM builds, flake checks, a NixOS consumer evaluation, or Home Manager module evaluation. The isolated expression above is not a substitute for those checks.
- Miri, fuzzing, a sanitizer, a dependency advisory or license audit, a lower-toolchain matrix, or aarch64 builds.
- A real browser session, slow-reader timing, queue-saturation behavior, caller-cancellation behavior, in-flight shutdown, whole-process crash containment, or filesystem fault injection.
- The ignored private-audio test or synthetic SSH test. No physical host, device, privileged helper, operating-system generation, firewall change, external listener, or public stream was touched.

Recommended negative-path evidence for later authorized local work is specific: SIGTERM and Stop during delayed Prepare; dropped answer receiver before execution; saturation by slow response consumers; a serve call that returns with no remaining worker threads or sockets; post-rename directory-sync failure; and out-of-order browser replies after token replacement. Tests must assert externally visible state and resource release, not private helper calls.

Historical green results remain historical. Do not add them to standalone test counts. No unrun check in this document is a pass. The parent session can append final verification with the final artifact identity, exact commands, outcomes, and any superseding fixes.

## RISKS & ROLLBACK

Disabled partial publication may retain the documented incomplete TTS, EventSub, shared selection transactions, GPUI, congestion recovery, and device or GPU acceptance work. These omissions must remain visible in Doctor, commands, and user documentation. Real capture, moving OBS preview, synchronization, camera and microphone provenance, game non-regression, listener isolation, forced-death cleanup, and aggregate shutdown latency still need their own acceptance evidence.

F01's reproduced export-shape failure is corrected. Full standalone NixOS integration remains unverified by this reviewer. F02 and F03 block a claim of graceful, cancellable runtime control. F04 through F07 block a claim of a stable, validated reusable API or accepted browser lifecycle. They need not be hidden or solved by broad refactoring to publish a clearly disabled experimental repository.

Preserve `rustCandidate = false` at `nix/modules/home-manager.nix:49-52`, `Install.WantedBy = []` at `112`, and the display's empty `wantedBy` at `nix/modules/headless-display.nix:67`. Keep optional display and firewall controls disabled. Module evaluation is not permission to install the capture helper or activate prerequisites. No build result authorizes runtime cutover.

This review's rollback is removal or amendment of `docs/design-review.md` only. No source, runtime state, or source evidence was changed by the reviewer. Implementation rollback belongs to the extracting session and must preserve unrelated work and verification failures. For a future deployment, Stop, verify actual descendant and remote-output release, and restore the approved prior artifact or generation only with separate operator authorization. There is no installed Python fallback to infer from historical documents.

## Extracting-session follow-up, 2026-11-14 JST

This section is added by the extracting session, not attributed to the independent reviewer. The independent source-preservation audit approved the disabled experimental extraction at `4166e7287b3b7fe2851ad97fa2ef372065ccdec9`. It checked all 124 original blobs/content hashes, coverage of all 120 source deletions, 76 publication snapshot hashes, source retirement, both Git diffs and publication hygiene. Its attention items were evidence reconciliation, operational account identifiers in historical copies, and the human documentation/policy licensing gate.

The completed verification is now in `docs/verification.md`. It records all 110 ordinary tests and the local matrix, the two opt-in local components, supply-chain checks, standalone native/WASM builds and flake checks, 22 module assertions and five real consumer gate assertions. These are parent-executed checks, not retroactively reviewer-executed commands. Full consumer flake evaluation still fails at the same Firefox theme derivation as the untouched source baseline; it is not reported green.

Actual consumer evaluation exposed a further integration defect absent from single-import module probes: anonymous Home Manager imports through both role bridges declared the same option twice. The export now has a canonical version-specific key and the NixOS export is a canonical file path. The earlier F01 function-argument probe is historical evidence for its original shape, not the final export representation. The consumer evaluator is retained as the regression check.

Operational account references in normalized historical copies were generalized. Repository attribution URLs remain intentional provenance. Original blob and content hashes remain unchanged, with current publication hashes recorded separately. The upstream OBS license and copyright notices are retained for the active patch and its historical copy. The human still confirms rights to publish imported documentation and operator policies.

No fix or verification result in this extraction closes F02, F03, F04, F05, F06 or F07. In particular, source publication is not a graceful-shutdown, cancellation, stable-library, live-browser, physical-capture or gameplay acceptance verdict.

The read-only follow-up approved disabled extraction at `7082111`, including the version-specific module key, unchanged Rust/Cargo application code from `a5a0a09`, upstream GPL byte identity and patch sidecars, all 124 raw identities, 120 deletion mappings and 78 current publication hashes. Final consumer gate and standalone flake-check jobs were still pending for that reviewer and are credited only to the extracting session's subsequent successful result inspection. Earlier standalone commits retain some source-account identifiers, so full-history privacy review remains a human publication gate despite current-tree normalization.
