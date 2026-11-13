# Rust agent entrypoint

This repository treats Rust design as a semantic contract, not a lint exercise.
Read `RUST_ENGINEERING_AGENTS.md` before nontrivial Rust implementation or review. Read relevant subsections rather than copying the entire document into every prompt when context is constrained.

Non-negotiable priorities:
1. Soundness, correct behavior, invariant preservation.
2. Explicit ownership and lifecycle of values, tasks, locks, files and remote effects.
3. Cancellation/failure atomicity, backpressure, graceful shutdown, structured errors.
4. Minimal public API, intentional abstraction, documented compatibility/MSRV.
5. Tests for negative paths and concurrency; measured performance before optimization.

Before coding: identify the domain invariant, owners, boundaries, observed baseline, side effects and cancellation points. Prefer concrete types and private fields; justify traits, clones, shared mutable state, additional crates and unsafe. Respect the project's toolchain, feature matrix and existing successes.

Before completion: run applicable fmt/check/Clippy/tests, independently state what was *not* run, show changed paths, remaining risks and rollback. Compiler success is not evidence of full semantic correctness.

For nontrivial review, use this report schema: CONTRACT / OWNERSHIP / STATE & ERRORS / ASYNC & SHUTDOWN / PUBLIC API / TRADEOFFS / VERIFICATION / RISKS & ROLLBACK.
