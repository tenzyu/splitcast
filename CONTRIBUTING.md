# Contribute to SplitCast

Read `AGENTS.md`, the two root Rust policies, and `MEMORY.md` before nontrivial work. Start with observable behavior, resource owners, failure and cancellation points, compatibility impact, and a verification plan.

Use the pinned Nix shell. Run `cargo fmt --all`, then `cargo run -p splitcast-xtask -- verify`. The verifier retains failures and runs every configured local check instead of treating the first successful compiler command as acceptance. Record the exact artifact and command when updating evidence.

Use focused failing-before and passing-after tests for cheap local regressions. A fixture failure, missing compiler, or unavailable device is not evidence of the intended bug. Preserve earlier observations when a new result changes a decision.

Do not add traits, shared ownership, generic layers, dependencies, or unsafe code without a concrete need. Unsafe blocks require a local proof obligation under `RUST_ENGINEERING_AGENTS.md`. Existing policy gaps are review findings, not examples to copy.

Keep role names and configurable tool paths portable. Run `cargo run -p splitcast-xtask -- portability` before publication. Do not commit tokens, passwords, private keys, generated recordings, personal machine aliases, or private raw captures.

Hardware tests, external services, privileges, firewall changes, operating-system activation, and public streaming need separate current authorization. Do not widen a test's permissions to force it green.
