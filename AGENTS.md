# SplitCast agent entrypoint

Read `RUST_AGENT_ROOT.md`, then the relevant sections of `RUST_ENGINEERING_AGENTS.md` before implementation or review. These operator-supplied policies are normative.

Read `MEMORY.md` and `docs/memory/INDEX.md` before resuming work. Read `docs/design-review.md` for the current semantic review and `docs/operations.md` before operational tests.

Use role names such as capture host and production host. Do not embed personal hostnames, home directories, credentials, or immutable local tool paths in runnable code or examples.

This is an incomplete experimental candidate. Historical success does not establish standalone or deployment success. Building or testing grants no permission to capture physical devices, publish a stream, install privileged helpers, switch an operating-system generation, or open public listeners. Keep the Nix runtime gate false and services without autostart targets until the operator separately authorizes deployment.

Use `nix develop -c cargo run -p splitcast-xtask -- verify` for local verification. Keep exact commands and results, including failures, in the verification record. Do not erase source evidence or turn an unrun check into a pass.
