# Resume GitHub Actions CI and Nix cache work

Prepared 2026-11-14 JST. This handoff is a plan for the next session. No workflow, hosted run, cache account, secret, cache upload, commit, or push was created by this documentation turn.

Implementation follow-up: S18 preserves the source at `7585308`, implements
hosted CI and a signed GitHub Releases cache, and records actual results in
[the CI/cache evidence](../evidence/ci-nix-cache-20261114.md). The operator chose
and approved Releases instead of the external Cachix account proposed below.
The original plan remains here for its validation and trust requirements.

## Goal and stopping boundary

Implement repeatable GitHub Actions validation and a usable SplitCast Nix binary cache. Keep CI acceleration separate from distributing package closures to users. Finish with actual hosted-run and clean-consumer evidence, or explicitly report the unavailable credential/account boundary. A workflow file or successful upload alone is not completion.

Do not deploy, capture devices, contact a real SSH peer, install privileged helpers, alter a host generation, open public listeners, or enable autostart. Use ephemeral GitHub-hosted Linux runners, not a capture or production host as an untrusted PR runner.

## Read and preserve the current work

Read `AGENTS.md`, `RUST_AGENT_ROOT.md`, `RUST_ENGINEERING_AGENTS.md`, `MEMORY.md`, and `docs/memory/INDEX.md`. Then read [the current-state capsule](../memory/session-state.md), [production-readiness gates](../production-readiness.md), and [the S16 verification record](../evidence/virtual-system-20261114.md).

Start with local inspection.

```sh
git status --short --branch
git rev-parse HEAD
git diff --stat
git ls-files --others --exclude-standard
```

At preparation time, HEAD is `aa86fe0d443d55a1a66c61198811a5b906ebef57`, branch `main`, with existing virtual-control source, tests, memory, and evidence changes still uncommitted. A new worktree from HEAD alone lacks the repaired implementation and `verify-virtual`. Review and deliberately preserve tracked and untracked files before forming the commit that CI will test. Do not use reset, broad checkout, or indiscriminate staging to obtain a clean tree.

Local status shows `origin/main`, but this turn did not inspect live GitHub identity, authentication, repository visibility, or branch protection. Resolve the actual repository and default branch through read-only Git/GitHub inspection. Do not run repository creation from the older publication recipe without checking whether the repository already exists. Do not invent an owner, cache URL, signing key, action version, or action commit SHA.

## Known baseline and input files

The last local extended run passed 14 steps, 124 ordinary tests, and three separate local components. It is evidence for the dirty source identified by [the manifest](../evidence/virtual-system-20261114-source-sha256.tsv), not a hosted CI result. Preserve V01, V02, and V03 corrections in the memory capsule. Review-only extra tests are not part of the 124-test total.

- `flake.nix` exports native `default`/`splitcast-rs-core`, `splitcast-web`, patched OBS, a stock OBS candidate, and VOICEVOX for `x86_64-linux` only.
- `nix/source.nix` explicitly includes source, workspace manifests, and fixtures. New source files must be tracked when using a Git-backed flake.
- `packages/splitcast-rs/default.nix` builds/tests the native crate and wraps it with WASM assets, RNNoise, FFmpeg, GSR, MediaMTX, PulseAudio, PipeWire, SSH, and patched OBS.
- `packages/splitcast-web/default.nix` builds WASM, tests web/protocol natively, and runs wasm-bindgen.
- `nix/obs.nix` applies the loopback patch. Do not replace the selected OBS package to make a build easier.
- `tools/xtask/src/main.rs` owns ordinary/virtual verification and per-run evidence. `nix flake check` does not invoke that entire matrix.
- No workflow files or project-declared cache endpoint/key were found during preparation.

## Implement the validation workflow first

Use a normal `pull_request`, trusted-branch `push`, and optional manual trigger. Give jobs read-only repository permissions unless a separately justified step requires more. Pin third-party actions to verified immutable commits when implementing the workflow. Resolve their current supported releases from their official sources at that time.

Use separate validation responsibilities, whether implemented as separate jobs or clearly separated steps.

| Responsibility | Commands or observations | Required result |
| --- | --- | --- |
| Dependency and toolchain preparation | Install Nix; inspect the actual Rust compiler and WASM target. Populate the Cargo registry with `nix develop -c cargo fetch --locked`. | Cold runner can execute the verifier's `--offline` checks. Do not assume `rust-toolchain.toml` configures Nix's compiler or supplies its WASM standard library. |
| Ordinary local matrix | `nix develop -c cargo run -p splitcast-xtask -- verify` | All 11 configured steps pass. Preserve this named entrypoint. |
| Extended virtual behavior | `nix develop -c cargo run -p splitcast-xtask -- verify-virtual` | All 14 steps pass, including nonempty opt-in checks. This command repeats the ordinary matrix; running both is intentional initial coverage, not additional unique tests. Consolidate later only if the explicit ordinary entrypoint and its evidence are preserved. |
| Standalone packaging | `nix build .#default .#splitcast-web --no-link --print-out-paths` and `nix flake check` | Native and WASM release packages and standalone checks pass on the actual CI commit. Record derivations, output paths, and lock identity. |
| Failure evidence | Upload an allowlisted set of verifier summaries and logs even on failure. | Original failure is visible. Avoid blanket upload of `artifacts/`, which can contain unrelated private archives or generated credentials. |

Set finite job deadlines and inspect cold-runner disk/time behavior. Patched OBS and the multimedia closure can be large. Disk or timeout failures are infrastructure observations to investigate, not permission to skip assertions or switch dependencies.

Fresh-runner risks that need execution, not assumptions:

- The development shell contains tools but does not pre-populate Cargo's registry.
- Rust 1.95.0, the WASM standard library, and wasm-bindgen must agree in the actual runner environment.
- Existing tests create subprocesses, sockets, private audio, and generated media. They do not require physical capture. Preserve their fixtures and isolation.
- `agent_lifecycle_tests.rs` currently generates a fixture script with `#!/bin/sh`. A Nix sandbox need not expose that path even when Bash is on PATH. If the package check fails, reproduce and fix the fixture portably, not by weakening the test. This is a source-inspected risk, not a reproduced failure.
- Module assertions in `nix/tests.nix` need an explicit pinned Home Manager input and are not wired into current flake checks. Plan their reproducible CI wiring separately; do not imply that `nix flake check` already runs them. The real consumer's unrelated Firefox evaluation failure is not a standalone CI failure.

## Add binary-cache publication with a separate trust boundary

Recommended initial service is Cachix to avoid operating a cache server. That is a recommendation, not a configured account or an approved public-upload destination. Use an existing approved Attic or other Nix cache if the operator prefers it. The next session must obtain the actual cache owner/name, public or private policy, endpoint, public signing key, and narrowly scoped write credential. Missing values block publication, not unprivileged validation. Never request a private key or token in chat.

An Actions dependency/store cache can speed up CI, but it is not proof that another machine can substitute SplitCast. Configure the consumer-facing binary cache explicitly.

Publication rules:

1. Untrusted PRs receive no cache-write credential, signing key, or deployment secret. They may read an approved public cache. Never execute untrusted PR code through `pull_request_target` with secrets, and never use a persistent personal host as its runner.
2. Publish only an approved default-branch or release revision after required validation succeeds for that exact SHA. Do not publish a PR-produced artifact or mismatched workflow-run SHA with privileged credentials. A secret-bearing publisher must not restore PR-writable build/store caches or artifacts. Keep acceleration-cache trust boundaries separate too.
3. Initially select only `.#default` and `.#splitcast-web`. Include their needed runtime closure. Native `default` already references patched OBS, web assets, RNNoise, and runtime tools. Do not build/push every output; VOICEVOX has explicit unfree dependency policy and is outside the initial cache scope.
4. Keep cache credentials out of Git, derivation inputs, Nix store paths, build logs, and uploaded artifacts. Configure real repository/environment secrets and verified public trust keys. Do not store placeholder credentials in workflow examples.
5. Keep publication behind the successful-validation condition. An automatic post-build upload hook must not bypass that condition. Separate cache-publication failure from a successful software test result in the final report.
6. Review publication rights and closure contents before public upload. The OBS patch retains its upstream GPL obligations. The native fileset excludes historical documents and local `artifacts/`, but that is not a complete redistribution/license audit.

Read [publication boundaries](../publication.md) before changing public visibility, source history, or consumer URLs. Do not rewrite history, force-push, or change runtime configuration as part of caching.

## Prove cache consumption

Use a second fresh ephemeral job or clean external consumer with the same source commit, `flake.lock`, architecture, and verified cache trust configuration. Do not delete a user's local store to simulate a cold consumer.

Confirm that the intended cache has the root output paths. Then prohibit local and remote fallback builds during the consumption check.

```sh
nix build .#default .#splitcast-web --no-link --print-out-paths --max-jobs 0 --builders ''
```

Record the output identities and provider/substitution evidence. A warm local store or upload exit zero is not a cache-consumption proof. If another cache could supply a path, query the intended cache directly as well. Validate the fetched native package with `selfcheck` and example `validate`, without starting an agent or capture. A consumer that changes Nixpkgs through `follows` may produce a different derivation; do not claim a hit for that variant from the standalone result.

## Completion record and next-session prompt

Require the actual workflow run URL and tested commit, each job's conclusion, retained failed attempts, native/WASM derivation and output identities, intended cache availability, a fresh no-build consumer result, and unchanged runtime/autostart gates. Update the memory evidence register with a new run identity. Do not overwrite S16's local result or call hosted tests, package builds, or cache consumption passed until each ran.

Start the next session with this request:

> Read `AGENTS.md`, `MEMORY.md`, `docs/memory/INDEX.md`, and `docs/handoff/ci-nix-cache.md`. Preserve the existing uncommitted implementation and evidence. Implement GitHub Actions CI, then the approved SplitCast binary cache, and prove substitution on a fresh consumer. Keep untrusted PRs unable to write the cache. If account credentials or publication approval are unavailable, complete and verify unprivileged CI first and report the publication boundary. Do not deploy, enable runtime/autostart, run physical capture, rewrite history, or erase earlier failures.
