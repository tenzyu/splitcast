# Standalone verification

This record belongs to the extraction workspace, not to the historical source report. Source baseline is `3c8857353581424e5cca517d7ccf0db8de7514d4`. The standalone repository is initialized on `main`, without a remote. No GitHub operation, physical capture, privileged helper installation, service activation, or public publication has been performed.

## Executed local checks

`nix develop -c cargo run -p splitcast-xtask -- verify` runs and records the declared local matrix. The initial workspace run passed native checks, tests, doctests, the no-default-features matrix, the valid all-features check, WASM target check, dependency trees, and portability. Warning-denied Clippy exposed a pre-existing needless lifetime in the now directly checked protocol crate. The lifetime was elided without a behavioral change. A rerun passed the full verifier.

The ordinary workspace includes the 102 inherited native tests, three protocol tests, three native web-model tests, one remote-argument validation test, and one repository-portability test. This is 110 passing tests, not 110 additional tests beyond the inherited suite. Three targets are ignored by default: real private audio, generated local media, and two-host synthetic transport. Doctest targets currently contain zero examples; their successful runs do not imply documentation-example coverage.

The isolated private-audio opt-in passed. A new generated-only local FFmpeg/FFprobe test passed recording, both codecs, 30 video packets, positive audio packets, approximately two seconds duration, and full decode. It uses software codecs and temporary files, not GPU capture or physical devices. These command outputs remain in `artifacts/verification/private-audio.log` and `local-synthetic.log`.

`cargo audit --json` reported no known vulnerabilities against advisory database commit `4ef2df8e806a65e2f1ecc14f56f51249062b23d6`. This is not proof that dependencies are vulnerability-free. `cargo deny check` initially rejected unversioned internal path dependencies under the explicit wildcard policy. Exact matching internal versions were added instead of suppressing the policy. The subsequent run passed; duplicate-version warnings remain informational and are recorded.

Before Git initialization, the portability check found two additional machine aliases in historical Nix logs. Their publication copies were normalized; original source identities remain preserved. After Git initialization, one Nix invocation failed because the new flake was not yet staged. Staging the intended files repaired source filtering. Neither failure is behavioral red evidence for runtime correctness.

The verifier now retains each run under a distinct `artifacts/verification/runs/` directory and updates `latest-run.txt` as an index. Earlier failed logs remain intact. Fresh builds, consumer integration, final artifact hashes and independent review of this extraction are recorded below as they finish. Historical store paths and historical successes are not promoted to this run.

## Pending or unrun acceptance

At the initial checkpoint, the standalone Nix build and consumer migration were pending. The completed results below supersede that pending status. A pinned local Git input remains a bootstrap until the owner publishes the repository and updates its URL.

No real two-host SSH trial was run with the newly parameterized harness. No physical GSR, microphone, camera, OBS decode/preview/recording, browser interaction, forced-controller-death, filesystem fault injection, or game-performance baseline was run. Miri, fuzzing, sanitizers, lower compiler versions and other architectures have not been exercised.

The [semantic review](design-review.md) retains blocking runtime limitations in whole-server cancellation and task joining, accepted-request outcome ambiguity, and narrower API/FFI/browser weaknesses. Passing local tests do not close those findings. This remains a disabled partial candidate.

## Completed extraction checks, 2026-11-14 JST

| Portable command form or check | Observed result | Scope |
| --- | --- | --- |
| `nix develop -c cargo run -p splitcast-xtask -- verify` | All 11 configured steps passed. [Command summary](evidence/standalone-20261114-summary.tsv). | 110 ordinary tests and the declared local feature/WASM/check matrix. Raw final run is `artifacts/verification/runs/1794582289292374056/`. Earlier failed logs remain preserved. |
| `nix develop -c cargo test -p splitcast-rs-core private_audio_dies --locked --offline -- --ignored --nocapture` | One isolated real-daemon test passed. | Ownership, readiness, process teardown, failed reconnect and socket removal, not physical audio. |
| `nix develop -c cargo test -p splitcast-rs-core --test local_synthetic --locked --offline -- --ignored --nocapture` | One generated-media test passed. | Local software H.264/AAC recording and full decode, not the agent's media transport. |
| `nix develop -c cargo audit --json` | Exit zero, zero known vulnerabilities; [database/result snapshot](evidence/cargo-audit-20261114.json). | Database commit `4ef2df8e806a65e2f1ecc14f56f51249062b23d6`, not a complete security proof. |
| `nix develop -c cargo deny check` | Passed after adding exact internal path dependency versions. | Advisories, bans, licenses and sources pass. Duplicate `base64` and `syn` versions remain warnings, not suppressed failures. |
| `nix build .#default .#splitcast-web --no-link --print-out-paths` | Both artifacts built. | Native package and WASM assets, including package checks; no activation. |
| `nix flake check` | Exit zero, all checks passed. | Includes generated capture/production configuration validation. Nix warns that `homeManagerModules` is an unknown conventional output; its module is evaluated separately below. |
| `nix eval --impure --json --expr '<nix/tests.nix with pinned nixpkgs, Home Manager and self>'` | All 22 Boolean assertions true. [Result](evidence/nix-module-assertions-20261114.json). | Real NixOS/Home Manager evaluators with a synthetic package, including negative options, gated/candidate commands, cgroup limits, optional display/firewall and preserved overrides. |
| `nix eval --impure --json --expr 'import ./nix/consumer-tests.nix { consumer = builtins.getFlake CONSUMER_GIT_URL; }'` | All five assertions true. [Role-only result](evidence/consumer-gates-20261114.json). | Actual consumer role settings and real external package identity, distinct from the synthetic module package. |
| Consumer `nix build .#checks.x86_64-linux.splitcast-configs --no-link --print-out-paths` | Exit zero; `/nix/store/1qslxi3gwjf2kbhpc8vf40df1ax9xfrj-splitcast-config-check`. | Independently consumed package/config check through the external input. |
| Installed `splitcast selfcheck`, `validate`, and offline `doctor` | Selfcheck and example validation succeeded; Doctor returned `ok=false`, no selected capture/game-audio source and `cutover_authorized=false`. | The refusal is the intended unselected-example result, not operational readiness. |
| Independent preservation and hygiene audit | All 124 original blob/content identities checked, all 120 deleted tracked files covered, and all then-current 76 publication hashes matched. | Original source Git history is unchanged; normalized snapshots are explicitly separate. Later publication normalization updates only the publication hashes. No secrets or legacy machine aliases were found in the reviewed committed snapshots. |

The direct standalone native artifact is `/nix/store/w9k4b02jxsq2d1nnkdjq1v00vaxkd8w3-splitcast-rs-core-0.1.0`. WASM assets are `/nix/store/9c6v0b1xpd2qyvlamcyj2vfp8aiw6anj-splitcast-web-0.1.0`. These are local artifact identities, not paths embedded in runnable configuration. The consumer's package variant and input revision are recorded separately by its gate evaluator. Do not assume a follows-adjusted consumer closure is byte-identical to the direct standalone closure.

The module evaluator accepts `{ nixpkgs, homeManager, self }`; the real consumer evaluator accepts `{ consumer }`. Expressions used local checkouts as inputs during this run. Published results retain role names only. The Home Manager source revision used by the module harness was `6eb2d33d1d0c95a37ef6443c88e6c1686012ee8b`. See [module evidence](evidence/nix-modules-20261113.md) for the separate earlier module-authoring probes and their limitations.

The expression placeholders in the table are portable descriptions, not claimed literal shell invocations. Exact executed expressions and shell wrappers are retained privately in `artifacts/verification/executed-commands.tsv`. The verifier's committed TSV separately records every literal Cargo argument vector. This avoids embedding personal checkout paths in published recipes while preserving the actual commands locally.

## Failures and observations that changed the plan

- The first directly checked protocol Clippy run rejected a needless explicit lifetime. Eliding it fixed the warning; no broad lint allowance was added.
- Cargo deny initially rejected two unversioned internal path dependencies. Exact matching internal versions fixed the policy violation. The audit itself had passed.
- The initial Git-backed Nix invocation could not see an unstaged new `flake.nix`. Staging the intended source fixed the filtering error. It was not a runtime failure.
- Direct single-import module tests passed, but the actual consumer initially failed with an already-declared `services.splitcast.settings` option. The independently imported anonymous Home Manager module needed a canonical, version-specific identity; the NixOS export now uses its canonical file path. Actual consumer role evaluation subsequently passed. This regression is why the separate consumer evaluator is retained.
- Full consumer `nix flake check --no-build` still fails while evaluating FirefoxColor settings, with invalid derivation `1rv3wnjgsnsg3nri8g271hxyknqin3as-catppuccin-firefox-0-unstable-2023-07-18.drv`. The identical error was reproduced by checking untouched source commit `3c8857353581424e5cca517d7ccf0db8de7514d4`. It is a baseline whole-consumer failure, not a passed check or an observed SplitCast migration regression. No unrelated Firefox or Nix-store repair was attempted. Raw outputs remain in `/tmp/opencode/splitcast-consumer-flake-check.log` and `/tmp/opencode/splitcast-consumer-baseline-flake-check.log`.

## Publication, remaining work and rollback

The independent semantic review and final extraction audit approve a disabled experimental source repository only. Runtime findings F02/F03 and API/FFI/browser findings F04–F07 remain visible in the review. The owner must confirm documentation/policy publication rights before creating the GitHub repository; upstream OBS patch licensing and license text are retained separately from MIT code.

The final implementation and module-export revision is `7082111295d1235bc85232fe76d165b18bff3483`. After the consumer input was repinned to that revision, all five actual consumer checks passed again, including its canonical/version-specific module identity and real package path. The independently consumed native package is `/nix/store/ab3yqdwpax0ra6pj5vqqqsjwic6hkzgr-splitcast-rs-core-0.1.0`. The standalone `nix flake check` also passed again at that implementation revision. Subsequent commits contain verification/publication records only; the consumer bootstrap deliberately pins the tested implementation revision.

A read-only independent follow-up approved disabled OSS extraction at `7082111`, rechecked all 124 raw identities, all 120 deletion mappings, all 78 publication snapshot hashes, module-key versioning, unchanged Cargo/Rust application code from `a5a0a09`, upstream GPL byte identity and sidecars, source retirement and hygiene. The reviewer did not claim the separately running final gate/check jobs had completed. Those jobs subsequently completed successfully and were inspected by the extracting session. No independent verdict is invented for a test the reviewer did not run.

The remaining consumer game-tool fixtures passed 11 launcher scenarios plus the timer helper and four draw-observe scenarios. These are hermetic stand-ins, not game or compositor observations. Raw output is `/tmp/opencode/splitcast-consumer-game-tools-tests.log`. Final repository portability passed, active documentation links resolved, and both Git diffs passed whitespace checks. The first link-check attempt accidentally included an obsolete historical README and was corrected to limit its claim to current documentation; historical links are not promoted to working current commands.

The publication audit also notes that seven files in earlier standalone commits retain source-account identifiers even though HEAD generalizes operational accounts. There are no remaining legacy machine aliases in the inspected commits, but full-history disclosure review remains a human publication gate. See `docs/publication.md` rather than assuming current-tree cleanup rewrote history.

No two-host, physical-device, GPU, browser, forced-crash, performance-baseline, Miri, fuzzing, sanitizer, lower-compiler or alternate-architecture result is claimed. Optional stock OBS and VOICEVOX package definitions were evaluated by flake checking, not separately built or accepted as live features. They are not selected implicitly.

Original source records remain recoverable from EXTRACT and the manifest's Git blobs. Ignored residual build/cache material was moved to the standalone repository's private `artifacts/source-checkout-residue/` rather than discarded or published. The consumer's implementation directories are removed. Its remaining changes are local, reviewable flake/module/settings and external-reference updates, with no consumer commit or push by the agent. [Publication steps](publication.md) require the human owner's authenticated GitHub operation.

Rollback preserves both histories. Build a prior standalone implementation in a separate checkout or restore the prior consumer input in a separate reviewed change. Inspect incomplete remote and cgroup cleanup before any runtime rollback. Do not reset unrelated work, infer an installed Python fallback, or activate a generation without separate permission.
