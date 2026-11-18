# SplitCast agent entrypoint

Read `RUST_AGENT_ROOT.md`, then the relevant sections of `RUST_ENGINEERING_AGENTS.md` before implementation or review. These operator-supplied policies are normative.

When resuming work that depends on prior decisions or evidence, read the compact `MEMORY.md`, then use `docs/memory/index.md` to select **only the relevant concept**. Older material pruned from HEAD is recoverable via `archive/pre-dreaming-20261117`; never preload the archive or use it as current runtime instructions. Read `docs/design-review.md` for the current semantic review and `docs/operations.md` before operational tests.

Use role names such as capture host and production host. Do not embed personal hostnames, home directories, credentials, or immutable local tool paths in runnable code or examples.

This is an incomplete experimental candidate. Historical success does not establish standalone or deployment success. Building or testing grants no permission to capture physical devices, publish a stream, install privileged helpers, switch an operating-system generation, or open public listeners. Keep the Nix runtime gate false and services without autostart targets until the operator separately authorizes deployment.

Use `nix develop -c cargo run -p splitcast-xtask -- verify` for local verification. Keep exact commands and results, including failures, in the verification record. Do not erase source evidence or turn an unrun check into a pass.

With direnv and nix-direnv installed, review `.envrc` and run `direnv allow` once. The environment disables stale-shell fallback and automatic lock-file writes. In OpenCode with the direnv shell plugin enabled, set the shell tool's `workdir` to this repository or a subdirectory and run commands directly (for example, `cargo run -p splitcast-xtask -- verify`); otherwise keep the `nix develop -c` wrapper. The global RTK plugin automatically rewrites supported shell-tool commands where permissions permit; RTK is optional and is not required by the devShell. A `cd` within one shell invocation does not reselect its environment.

For a bounded multi-step change, use the [convergence workflow](docs/convergence.md)
to pin the accepted scope and reuse current receipts before rerunning checks.
Continue to consult the [current gates](docs/memory/session-state.md) when a task
touches physical acceptance. Dreaming proposals are opt-in.
