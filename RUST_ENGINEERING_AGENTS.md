# Rust Engineering Doctrine for Coding Agents

**Status:** normative review policy (project-specific requirements override preferences, not safety invariants).  
**Purpose:** make Rust changes that are semantically correct, sound, comprehensible, operationally safe, and verifiable.  
**Scope:** Rust libraries, applications, daemons, CLIs, async services, and FFI; apply domain-specific addenda when relevant.

## 0. Operating contract

Before editing, inspect the relevant repository code, `Cargo.toml`/`Cargo.lock`, toolchain configuration, active feature sets, tests, and existing architectural conventions. Identify the intended behavior and constraints. Distinguish observations from assumptions. Preserve existing successful behavior. Do not invent missing runtime facts.

The ordering of concerns is: (P0) soundness, semantics, invariant preservation; (P1) ownership and resource lifecycle, failure/cancellation atomicity, compatibility, concurrency; (P2) abstraction economy, performance evidence, observability; (P3) style. A lower-priority improvement never justifies a higher-priority regression.

For nontrivial changes, state: (a) invariant; (b) owner of data, tasks and resources; (c) error/cancellation model; (d) design alternatives and reasons rejected; (e) compatibility impact; (f) tests and rollback. Prefer the smallest sufficient implementation. Never silently make a scope expansion or destructive migration.

## 1. Correctness and modeling

- Write observable behavior and failure modes before choosing structs, traits or crates. Differentiate safety (nothing bad happens) from liveness (something good eventually happens).
- State invariants in terms of values and state transitions, not comments alone. Use enums for mutually exclusive states; use newtypes for semantically distinct identifiers, units, constrained values and validated inputs; keep invariant-bearing fields private.
- Prefer a fallible `TryFrom`/constructor for validation and a total `From` conversion only when no failure is possible. Validate at trust boundaries, including deserialization and FFI, and convert into validated domain types. Do not assume `derive(Deserialize)` on a public-field struct preserves domain invariants.
- Choose run-time state enums for state machines that need persistence, dynamic transitions or heterogeneous storage. Consider typestate only when the compile-time phase separation materially simplifies the API; avoid typestate explosion and opaque generics.
- Keep business rules deterministic and independently testable where practical. Separate domain policy from I/O, clocks, environment, persistence, transport and user interaction. Do not introduce a multi-layer architecture if a module boundary suffices.
- Prefer exhaustive `match` for protocol/domain states. Never use `_` to silently ignore important future variants. Represent policy decisions with named enums instead of positional booleans or magic strings.
- Distinguish protocol wire representation from validated domain types. Specify versioning, defaulting, size limits, unknown fields, backward-compatibility and migration behavior.

## 2. Ownership, borrowing, lifetime and allocation

- Start from a single, identifiable owner; pass `&T` or `&mut T` for short-lived access, transfer `T` when ownership changes. The question is WHO owns the value and WHEN it is released.
- Accept `&str`, `&[T]`, and `&Path` for read-only borrowed input when sufficient. Return owned values when the result must outlive its source. Avoid lifetime parameters unless they encode a real borrowing relation with a meaningful payoff.
- Treat `Clone` as an explicit ownership or snapshot decision, not a reflexive fix for E0382/E0502. Inspect allocations and deep copies; `Arc::clone` is refcounted sharing, not a deep copy.
- `Box<T>` for indirection/recursive types or intentional size/ownership boundaries; `Rc<T>` for single-thread shared ownership; `Arc<T>` for thread-safe shared ownership. None should be the default data-modeling primitive.
- Avoid `Arc<Mutex<Everything>>` as an architecture. Prefer explicit ownership, actor/task ownership, message passing, or carefully scoped shared state. Never hold a blocking mutex guard across an `.await`.
- `Cow`, arenas, interning, pooling, `SmallVec`, `Bytes`, `Arc<str>` and zero-copy parsing require evidence of actual lifetime/allocation pressure. Do not trade maintainability for speculative micro-optimization.
- `Pin` is for address-sensitive invariants, most notably certain futures and intrusive structures; it is not a general solution to ownership or lifetime errors. Explain `Unpin` and projection constraints before introducing manual pinning.

## 3. APIs, abstraction and compatibility

- Default to private implementation details. Use `pub(crate)` or `pub(super)` where sufficient; export deliberately via `lib.rs`. A `pub` item is a long-term promise to downstream code.
- Follow Rust API Guidelines for naming, `as_`/`to_`/`into_`, `iter`/`iter_mut`/`into_iter`, standard conversion traits, and useful standard traits. Implement `Debug` for public types unless there is a concrete reason not to; redact secrets.
- Prefer inherent methods and concrete types until independent implementations, generic algorithms, or a stable public extension seam justify a trait. Do not add repository/service/factory/provider traits solely for mocks.
- Decide polymorphism intentionally: generics/`impl Trait` permit static dispatch and monomorphization; `dyn Trait` permits runtime substitution/type erasure and may limit inlining while reducing code duplication. Assess dyn compatibility and object lifetime/auto-trait bounds before choosing it.
- Keep trait surfaces small and semantically coherent. Evaluate coherence/orphan-rule implications, blanket implementations and downstream trait impl commitments. Consider sealed traits when downstream implementations must not be permitted.
- Keep public struct fields private if validation/evolution matters. Consider `#[non_exhaustive]` at type introduction where forward extensibility matters; do not add it casually to an established type.
- Avoid large builders for trivial construction. Prefer simple constructors for small required argument sets and builders for nontrivial optional configuration or ordering constraints.
- Define MSRV and supported platforms/targets. Check public signatures, feature flags, error types, `Send`/`Sync`, CLI flags, config format and wire format when changing compatibility. SemVer applies beyond signature changes.
- Do not split a crate solely because a folder grew. Split where there is a real reuse/dependency/API/security/build boundary. Dependency direction must remain intelligible.

## 4. Errors, failure, side effects and resources

- A recoverable failure is `Result`, absence is `Option`, an impossible-by-invariant state can justify a documented panic. Do not represent ordinary I/O, parse, network or user errors as panics.
- For reusable libraries and public domain APIs, use structured error variants, `std::error::Error`, and causal `source()`. At an application orchestration boundary, a contextual report type may be appropriate. Preserve actionable distinctions; do not reduce all errors to strings or silently turn errors into defaults.
- Classify failure: invalid input, transient/retryable, permanent, cancellation, timeout, conflict, integrity violation, programmer bug. Specify the caller's recovery action and which errors are observable externally.
- Every `unwrap`/`expect` in production code must be justified by a local, documented invariant; prefer a precise `expect` message when a proof is legitimate. Never disable a lint or discard a `Result` merely to make CI green.
- Keep side effects explicit. Specify commit points, idempotency keys, retry policy, backoff/jitter, timeout, partial writes, duplication, transaction boundaries, durability guarantees and compensation. Exactly-once effects do not follow from a retry loop.
- `Drop` is for non-failing, generally bounded local cleanup. A destructor cannot use `await`; don't rely on it to perform complex async graceful shutdown or guaranteed persistence. Provide explicit `close`, `shutdown`, `flush`, or `commit` that returns a `Result` when completion/failure matters.
- Model file replacement and DB updates as failure paths, not just happy paths. `rename` alone is not a universal crash-durability guarantee; consider flush/fsync, filesystem, directory sync and platform semantics if durability is required.
- Secrets must not appear in `Debug`, logging, error displays, metrics labels, panic messages or snapshots.

## 5. Async, concurrency and cancellation

- Choose synchronous code unless concurrency or asynchronous I/O is needed. Async is a scheduling model, not automatically parallel CPU execution.
- Describe each task's owner, creation site, exit condition and join/shutdown path. Avoid detached `tokio::spawn` unless intentionally fire-and-forget and failure is handled. `JoinHandle` drop does not by itself cancel a Tokio task; `JoinSet` drop aborts its tracked tasks.
- Bound work queues, task fan-out, memory usage and CPU parallelism. State the backpressure policy (wait, reject, drop, coalesce) and fairness/starvation expectations. Never assume an unbounded channel is free.
- Audit all `.await` points for cancellation safety and holding of mutable state/guards. `tokio::select!` drops losing branch futures. `read_exact`, `read_to_end`, `read_to_string` and `write_all` are not cancel safe for restart-in-a-loop use; use persistent buffers/state or a dedicated owned task where needed.
- Keep blocking system calls and CPU-heavy work off cooperative executor threads. `spawn_blocking` is not reliably abortable once started; bound its concurrency or use a dedicated thread/pool when appropriate.
- Prefer short critical sections and `std::sync::Mutex` for synchronous data access not held across `.await`; Tokio's async mutex is useful when an async guard is genuinely needed (but avoid lengthy lock holds).
- For atomics, document the happens-before argument. `Relaxed` guarantees atomicity but not cross-variable publication; `Release`/`Acquire` synchronize under applicable reads-from relationships. Start with mutex/channel unless atomic coordination is justified.
- Shutdown protocol: stop accepting work -> request cancellation -> drain or rollback in-flight state -> flush/close resources -> join owned tasks -> enforce a documented timeout. Cancellation tokens express a request, not completed cleanup.
- Tests must include task failure, dropped receiver, caller cancellation, timeout race, early shutdown, queue saturation, poison/recovery as applicable, and load fairness.

## 6. Unsafe, FFI and layout

- Default to safe Rust. Any `unsafe` addition requires a written justification for why safe abstractions are inadequate, and a proof obligation naming each invariant: aliasing, lifetimes, validity, alignment, initialization, layout, provenance, thread safety and unwinding where applicable.
- Keep unsafe blocks small and adjacent to `// SAFETY:` explanations that cite the precise local invariant. `unsafe fn` must document `# Safety` caller obligations. `unsafe impl Send/Sync` requires a manual invariants audit.
- Safe public wrappers around unsafe internals must be sound for *all valid safe callers*, not just the current call sites. No safe caller can be required to uphold an undocumented memory-safety precondition.
- `#[repr(C)]` and FFI ABI contracts require scrutiny of C layout, padding, alignment, ownership, string encoding, nullable pointers, callbacks, thread affinity, panic/unwind behavior and dynamic library lifetime.
- Do not assume the Rust aliasing model is completely settled; use the Reference and Rustonomicon for established obligations. `cargo +nightly miri test` is a useful UB detector but not a soundness proof or substitute for reasoning.

## 7. Data formats, security and dependency policy

- Treat external bytes, user parameters, environment variables, paths, responses and config as untrusted. Validate length, ranges, encodings, nesting depth and resource limits before expensive work. Avoid path traversal, unsafe temporary files and unbounded decompression.
- Do not add dependencies without a need and a maintenance/security/licensing review. Check feature requirements, MSRV, duplicates, default features and whether std already solves the task.
- Cargo features must be additive; test default, `--no-default-features`, and supported feature combinations. `--all-features` is a valuable check only when the combination is valid. Audit feature unification across the dependency graph.
- Preserve lockfile policy (`Cargo.lock` ordinarily committed for applications, context-dependent for libraries) and supported toolchain pinning. For edition 2024, understand resolver 3 and MSRV-aware resolution; a virtual workspace must declare its resolver explicitly.
- Use supply-chain checks where configured (`cargo deny check`, `cargo audit`, trusted sources). These detect known issues, not all vulnerabilities; never assert an unverified package is safe.

## 8. Performance and observability

- Require a workload, baseline, target metric and regression threshold before substantial optimization. Compare throughput, p50/p95/p99 latency, tail behavior, allocations, peak RSS, CPU time, I/O, build time and binary size as relevant.
- Investigate algorithms and data ownership before choosing a faster map, allocation trick, custom allocator or unsafe code. Distinguish latency from throughput and capacity; account for lock contention, scheduler overhead, cache misses and memory bandwidth.
- Benchmarks must use representative sizes and distributions, correct optimization profiles, warm-up/repetition, environment controls and meaningful statistical spread. Never call `cargo check` a performance test. Avoid indiscriminate `#[inline]`, LTO, `unsafe` or `clone` avoidance.
- Structured logs/traces should expose operation IDs, transitions, error causes, time spent and queue depth, without leaking secrets. Avoid unbounded/high-cardinality labels.

## 9. Testing and evidence gates

- Validate invariants through unit tests, behavioral contracts through integration tests, public examples through doctests, and state-machine properties through property-based testing when useful.
- Test negative paths and boundaries: empty input, maximum size, duplicate identifiers, malformed UTF-8, integer overflow, unusual paths, partial I/O, EOF, retries, cancellation, panics, and shutdown.
- For tricky interleavings, use deterministic concurrency tests or `loom` where the code can be modeled correctly. For unsafe code, add Miri and relevant platform tests. For parsers, consider fuzzing and corpus regression.
- Never report a check as passed unless it actually ran. Inspect failures before suppressing diagnostics; do not insert blanket `#[allow]`, `todo!()`, or `unimplemented!()` in production paths.
- Review code coverage as evidence of specific properties, not a success metric on its own. Include compile-fail tests when invalid APIs are supposed not to compile.

### Baseline verification (adapt to repository; preserve toolchain)

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo test --workspace --doc --locked
cargo check --workspace --all-targets --all-features --locked  # only if valid features compose
cargo tree -d
cargo tree -e features
```

`cargo fmt --all -- --check` does not change files. `cargo clippy -- -D warnings` can fail because of pre-existing warnings; report the baseline separately instead of suppressing blindly. The all-features check does not replace a supported feature matrix. Do not run commands blindly in a `nix develop` repository; use the repo's declared environment or `nix develop -c ...` when required, and report commands run.

### Specialized checks (only if installed/relevant)

```sh
cargo +nightly miri test   # unsafe/UB-relevant; target and FFI support vary
cargo deny check           # needs cargo-deny/config and advisory access
cargo audit                # needs cargo-audit and advisory access
cargo test --no-default-features  # choose a package/feature matrix as needed
```

## 10. Required review output

For every nontrivial change, report:
1. **Behavioral contract:** exact requirements, invariants, success/failure states.
2. **Design decisions:** ownership graph, module/API boundary, alternatives, why chosen.
3. **Failure analysis:** error taxonomy, retry/cancel/timeout/shutdown, data consistency.
4. **Compatibility and risk:** public API, features, MSRV, dependencies, unsafe, migrations.
5. **Implementation:** minimal changed paths and rationale; no unrelated refactors.
6. **Verification:** exact commands run, pass/fail outcomes, test cases, unrun checks.
7. **Residual risks and rollback:** what is unknown, how to detect failure, how to restore baseline.

## 11. Red flags requiring deliberate justification

- `Arc<Mutex<Everything>>`, broad mutable globals, detached tasks, and unbounded channels.
- Blanket `Clone`, `Box<dyn ...>`, `async_trait`, `Pin<Box<...>>`, or lifetime parameters introduced only to placate the compiler.
- `String` errors for public library contracts; `.ok()`, `.unwrap_or_default()` on meaningful failures; ignored `Result`s.
- Trait for every struct, `Repository`/`Service`/`Manager` abstractions without an actual boundary, many crates without independent ownership.
- Public fields on invariant-bearing types; externally implementable traits with no compatibility plan.
- `unwrap` on untrusted data; `unsafe` without proof; unsound `Send`/`Sync` impls.
- Synchronous blocking in async hot paths, cancellation-unsafe `select!`, guards across `.await`.
- Success-only tests, benchmark claims without numbers, superficial `cargo check` as proof of behavior.
- Feature additions that break additive unification; dependencies that silently raise MSRV.
- Protocol/data persistence changes without schema/version/migration/corruption tests.

## 12. Sources to consult before overriding a rule

- Rust API Guidelines: https://rust-lang.github.io/api-guidelines/
- The Rust Reference / UB: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
- The Rustonomicon: https://doc.rust-lang.org/nomicon/
- Rust Edition Guide (2024): https://doc.rust-lang.org/edition-guide/rust-2024/
- Clippy: https://doc.rust-lang.org/clippy/
- Cargo Book (features, resolver, semver, workspaces, tests): https://doc.rust-lang.org/cargo/
- Tokio select/cancellation: https://docs.rs/tokio/latest/tokio/macro.select.html
- Tokio task lifecycle: https://docs.rs/tokio/latest/tokio/task/struct.JoinSet.html
- Tokio graceful shutdown: https://tokio.rs/tokio/topics/shutdown
- Miri: https://github.com/rust-lang/miri
- Loom: https://docs.rs/loom/latest/loom/
- cargo-deny: https://embarkstudios.github.io/cargo-deny/

**Decision rule:** A guideline may be overridden with a concrete, documented reason, but never treat a compiler success, Clippy pass, or plausible architectural pattern as evidence that the system's semantic, concurrency, durability, or API-compatibility contracts hold.
