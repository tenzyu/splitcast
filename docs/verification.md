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

## Virtual-software follow-up, 2026-11-14 JST

The current run starts at `aa86fe0d443d55a1a66c61198811a5b906ebef57` plus the uncommitted virtual-control changes. Historical extraction results above retain their artifact identities. The new [verification record](evidence/virtual-system-20261114.md) preserves commands, failures, negative controls, independent review scopes, and remaining gaps.

`nix develop -c cargo run -p splitcast-xtask -- verify` passed all 11 steps. After retaining the independent running-SIGTERM probe as a second ordinary two-agent test, `nix develop -c cargo run -p splitcast-xtask -- verify-virtual` passed all 14 steps with 124 ordinary tests and three separately executed local opt-in components. See the [command summary](evidence/virtual-system-20261114-summary.tsv) and [tested source manifest](evidence/virtual-system-20261114-source-sha256.tsv). Raw final logs are in `artifacts/verification/runs/1794622014178620022/`.

The software checks now cover actual local capture and production agents with stand-in workers, owner isolation, repeated recording Start, selected worker failures, explicit recovery, SIGTERM while Running, expired queued requests, running-outcome ambiguity, connection closure and joins, OBS compound cancellation, and generated delayed TCP transfer with disconnect, cleanup, and full decode. Independent integrated runtime reviewers returned no verified scoped defects.

This does not complete SplitCast. Ordinary Stop remains behind active Prepare. Actual agent RTMP and RTSP media integration, congestion recovery, durable outcomes, settings fault injection, browser lifecycle, and physical capture and performance acceptance remain separate work. No new Nix package build, consumer evaluation, deployment, real remote SSH, physical device, real OBS acceptance, or game-performance claim is made. Runtime and autostart gates remain unchanged.

## CI and binary-delivery follow-up, 2026-11-14 JST

S18 preserves the formerly uncommitted source in `7585308` and independently
builds/tests it through hosted CI. Its [CI/cache record](evidence/ci-nix-cache-20261114.md)
separates actual hosted jobs, local reruns, package outputs, cache export failures
and corrections, source/rights review, and public-consumer evidence. GitHub
Releases supplies the cache transport. No external cache account is assumed.
Earlier statements about no remote, dirty source, or no new package build apply
only to their original checkpoints. No delivery result accepts physical capture,
software media-path integration, browser behavior, host activation, or gameplay.

## PR #2 control-plane follow-up, 2026-11-14 JST

The S16 Stop-behind-Prepare statement above describes S16, not the
`review/urgent-control` candidate. Implementation `5125799` preserves one effect
owner while admitting Stop and inbound lease maintenance independently.
[The review record](evidence/urgent-control-review-20261114.md) records expired
owner cleanup and pre-dispatch remote-obligation regressions, focused outcomes,
and observation/replay/admission limits. It preserves the failed fixture runs.

[PR #2](https://github.com/tenzyu/splitcast/pull/2) records fresh final-head
`verify`, `verify-virtual`, ten admission cases, their run identities, and hosted
CI. Earlier matrices in this file are not promoted to results for that head.
Physical acceptance, actual agent media integration, browser lifecycle, durable
history, aggregate shutdown bounds, and deployment remain unverified here.

## Production-only media follow-up, 2026-11-15 JST

S20/E16 tests corrected implementation `42a64e41990eb3a411dcd68e3412ccceac574112`, based on actual main `2a774d9`, which already includes PR #2. This supersedes the older missing-integration statement only for the generated production-only path. [The receipt](evidence/media-plane-20261115.md) records source and binary hashes, exact commands, failed attempts, observations, and remaining gates. AV01 supersedes the initial `0accf63` phase-fit timing claim.

- `nix develop --no-write-lock-file .#media-test -c cargo run -p splitcast-xtask -- verify-media` passes eleven pure oracle tests, all six actual-media cases and three recording-mutation A/V controls at `42a64e4`, run `20261115T022547Z-vrJRNZQD`. Real controller-owned services create independently decoded OBS recordings. Stale-content rejection, disconnect, stall, worker death, release and same-controller recovery pass.
- `nix develop --no-write-lock-file -c cargo run -p splitcast-xtask -- verify-virtual` initially exits 1 at `42a64e4`, run `1794709924594226389`: thirteen stages pass, but no-default-features fails at initial Prepare in `late_owned_start_cannot_recreate_a_completed_remote_session` with a connection reset. Its cause is unknown. The single retry and final ordinary results are recorded in the receipt.
- The same `verify-virtual` command's single retry passes all 14 stages at unchanged `42a64e4`, run `1794710203786488803`: 155 ordinary tests in each feature selection and three separate one-test local components. The original reset is retained as unexplained, not declared fixed.
- `nix develop --no-write-lock-file -c cargo run -p splitcast-xtask -- verify` passes all 11 stages at unchanged `42a64e4`, run `1794710355626646567`, including both feature selections.
- Earlier `0accf63` `verify` run `1794692996286907638` passes 11 checks; `verify-virtual` run `1794692450019307764` passes 14 with 155 ordinary tests and three local opt-ins. These are preserved historical results, not substitutes for the final-source commands. Doctests contain zero examples.

The first baseline was red on ignored direnv symlinks, the first unsandboxed media attempt failed device isolation, and the first full real-media matrix was red. They remain recorded rather than rewritten as passes. Later documentation commits preserve proof for the tested implementation; they are not additional runtime executions.

No capture-Agent/SSH, actual network delay, live freshness enforcement, microphone alignment, default-VAD speech quality, physical capture, game-performance, deployment, fresh Nix package/consumer build, advisory scan, Miri, fuzzing, or alternate-architecture result is claimed. P1 is narrowed, not closed. Runtime and autostart gates remain disabled.

## 2026-11-17 PR #3 review follow-up

Implementation `fb632bf245b7e666ce9330b25c907c80d59496cc` has separate evidence
in [S22/E17](evidence/pr3-review-20261117.md), including the old oracle accepting
the same real VFR/dropout files now rejected, complete PCM-window boundaries,
restored null worker stderr, enforced source health, and twenty bounded reset
attempts. A failed VFR-fixture command and its one correction remain recorded.

- `nix develop --no-write-lock-file .#media-test -c cargo run -p splitcast-xtask -- verify-media`:
  exit 0, `20261117T083058Z-ApaTam5y`, sixteen oracle tests, six cases and six
  recording controls, cleanup and same-controller recovery PASS.
- `cargo run -p splitcast-xtask -- verify`: exit 0 in the existing pinned default
  Nix/direnv shell, run `1794904247771515533`, eleven stages PASS.
- `cargo run -p splitcast-xtask -- verify-virtual`: exit 0 in that same shell,
  run `1794904349298835682`, fourteen stages PASS. 156 ordinary tests per feature
  selection; three additional one-test opt-ins are separate. No reset recurred.

These are implementation-head results; final documentation-bearing HEAD's three
commands and exact receipts are published separately in
[PR #3](https://github.com/tenzyu/splitcast/pull/3). The source manifest excludes
documentation, not executable inputs. No historical result is relabeled and no
new physical, speech-return, delivery or deployment gate is closed.
