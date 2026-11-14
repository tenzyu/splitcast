# Virtual system verification record

This run starts at standalone revision `aa86fe0d443d55a1a66c61198811a5b906ebef57` on `main`. The initial working tree was clean. Subsequent results describe the uncommitted changes in this task, not an installed package or operating-system generation. The date is 2026-11-14 in Japan Standard Time. The local UTC clock was 2026-11-13.

## Commands and observed outcomes

| Exact command | Observed outcome | Evidence and scope |
| --- | --- | --- |
| `nix develop -c cargo run -p splitcast-xtask -- verify` | Exit 1. All Rust matrix steps passed. Portability failed. | `artifacts/verification/runs/1794600544644074493/`. The existing evidence JSON records a Nix store identity. No application implementation had changed. |
| `nix develop -c cargo test -p splitcast-xtask --locked --offline` | Exit 0. Two tests passed. | Evidence JSON is not runnable configuration. A runtime JSON containing the same fixed path is still rejected. |
| `nix develop -c cargo test -p splitcast-rs-core control::tests::slow_response_consumer_cannot_renew_the_write_budget --locked --offline -- --nocapture` | Exit 101 before the fix. | The assertion printed `write budget renewed for 2.003574555s` for a 200 ms budget. A short-deadline entry point exposed the existing per-write behavior. |
| `nix develop -c cargo test -p splitcast-rs-core control::tests --locked --offline -- --nocapture` | Exit 0 after the write fix. Seven tests passed. | The slow-reader regression passed. A lost-response test observes a received Start request and an unknown client outcome. |
| `nix develop -c cargo fmt --all` | Exit 0. | Formatting only. Repeated after later edits. |
| `nix develop -c cargo run -p splitcast-xtask -- verify-virtual` | Exit 1 at the intermediate checkpoint. | `artifacts/verification/runs/1794601032574388832/`. Fmt and Clippy failed on the new xtask formatting and Boolean expression. Tests, private audio, generated recording, generated transport, and portability passed. No failed log was replaced. |
| `nix develop -c cargo run -p splitcast-xtask -- verify-virtual` | Exit 0 after the xtask fixes. All 14 steps passed. | `artifacts/verification/runs/1794601163685775387/`. This checkpoint has 114 ordinary passing tests and three separate local opt-in passes. It precedes integration of the server-lifecycle and two-agent changes. |

The portability repair preserves the original JSON evidence. Only JSON under `docs/evidence/` is exempt from runnable-path checks. Shell scripts in that directory and runtime configuration remain checked. Machine-alias scanning remains active everywhere it was active before.

## Delegated checks

The media-test worktree ran `nix develop -c cargo test -p splitcast-rs-core --test local_transport --locked --offline -- --ignored --nocapture` twice successfully. Its full verifier still failed on the pre-existing portability classification. These delegated results are not substituted for checks in the primary checkout. The primary extended matrix independently ran the generated transport test successfully in the intermediate checkpoint above.

The first two-agent delegate ran `nix develop -c cargo test -p splitcast-rs-core --test two_agents --locked --offline -- --nocapture` before creating the test target. Cargo exited 101 with the target missing. This was a fixture/setup failure, not behavioral red evidence. Its logs remain in `/tmp/opencode/splitcast-two-agent-check-20261114-59076/`. A new worker owns the implementation. No success is inferred from that first attempt.

## Open verification

Server lifecycle implementation, two-agent integration, final matrix, independent review, and artifact reconciliation remain pending at this checkpoint. Final results must be appended, not inferred from passing components.

No real remote SSH host, physical device, GPU capture, existing audio graph, real OBS instance, public listener, public stream, service activation, privileged-helper installation, or operating-system switch was used. No new Nix package build, consumer evaluation, performance benchmark, live browser interaction, Miri, fuzzing, sanitizer, lower-toolchain check, alternate architecture, or forced-controller-death trial is claimed.

## Independent adversarial findings

V01 is a client outcome-classification defect. Sol and Luna independently returned an HTTP 500 after receiving a command and observed `control command rejected; no redirects permitted`, without an unknown outcome. Sol's retained reproduction is `packages/splitcast-rs/tests/review_http_outcomes.rs` in its review worktree. The exact command was `nix develop -c cargo test -p splitcast-rs-core --test review_http_outcomes --locked --offline -- --nocapture`, exit 101 on both runs. Luna's temporary probe used `nix develop -c cargo test -p splitcast-rs-core --test review_http_probe --locked --offline -- --nocapture` and printed the same classification. These findings share one cause and count as one issue.

The primary checkout reproduced V01 with `nix develop -c cargo test -p splitcast-rs-core control::tests::rejects_redirect_and_status_code_substrings --locked --offline -- --nocapture`, exit 101. It printed `control command rejected; no redirects permitted` instead of an unknown outcome. Unrecognized statuses now report unknown and still never follow redirects. The first broader rerun exposed incomplete request consumption in the response fixture, not another client defect. The fixture now reads the complete authenticated request before responding. The subsequent `nix develop -c cargo test -p splitcast-rs-core control::tests --locked --offline -- --nocapture` passed all eight tests. V01 is fixed with regression coverage.

V02 is an empty-test verification defect. Terra renamed the private-audio test in its review worktree, leaving the verifier filter unchanged. `nix develop -c cargo run -p splitcast-xtask -- verify-virtual` exited 0 and reported `private-audio: PASS` even though `artifacts/verification/runs/1794601463816881983/private-audio.log` recorded zero tests. A separate mutation removed the transport test's ignore attribute and also produced a dedicated zero-test pass. Both mutations were restored in that review worktree.

The verifier now requires at least one executed passing test whenever it invokes Cargo with `--ignored`. This derives the requirement from the command rather than maintaining a second list of check names. `nix develop -c cargo test -p splitcast-xtask --locked --offline` passed three tests after the fix, including rejection of empty and filtered-out results. An end-to-end mutation check remains pending. V02 is implemented but not yet closed by that stronger check.

The comment-only review found no new suppression or code-comment deletion candidate. This is not a correctness verdict. Review findings are retained separately from test failures and hardware acceptance.

## Integrated checks

`nix develop -c cargo test -p splitcast-rs-core --test two_agents --locked --offline -- --nocapture` passed in the primary checkout with one passing scenario and one ignored helper entrypoint. Its role configs, worker PIDs, output-command log, and agent stderr remain under `artifacts/verification/two-agents-106256/`. It exercised four explicit Prepare and recording sessions, repeated Start without duplicate StartRecord, conflicting owner refusal, capture and production worker death, cleanup, and explicit recovery. Seven helper PIDs and the relevant listeners were released. The control peer was the actual production agent, not a fake Peer. SSH and media workers were stand-ins.

After integrating the lifecycle changes, `nix develop -c cargo run -p splitcast-xtask -- verify-virtual` exited 0 with all 14 steps passing. The run is `artifacts/verification/runs/1794602044658804356/`. The ordinary suite passed 123 tests. Three additional opt-in component checks ran separately. Five tests were ignored in the ordinary suite, including the fixture-only worker entrypoint and the separately authorized real SSH trial. Zero doctests exist. The no-default-features rerun is not another set of unique tests.

The integrated compiler is Rust 1.95.0 for `x86_64-unknown-linux-gnu`, with LLVM 21.1.8. The unchanged Cargo lockfile Git blob is `0fcbbdafa7b551161d3ff595e6f2c4ec878a4fc6`. `artifacts/verification/virtual-system-source-sha256.tsv` records 49 source and configuration identities for this integrated dirty tree. This is not a Nix package artifact identity.

The coordinating session read every delegated source file before integration and compared the integrated lifecycle files against the worker's actual bytes. The only difference at that comparison was indentation of two existing match arms. Delegated command records and failure logs were copied without replacing earlier logs to `artifacts/verification/delegated/control-lifecycle/`, `two-agent/`, and `media/`.

The lifecycle worker's command record includes an intentionally broken ticket gate. `nix develop -c cargo test -p splitcast-rs-core --locked --offline expired_and_abandoned_queue_entries_never_write_settings -- --nocapture` failed because the mutated code actually wrote forbidden settings. The mutation was restored. Earlier missing-Origin fixture failures and formatting failures remain in its record and are not described as behavioral red evidence.

The stronger V02 negative control reran `nix develop -c cargo run -p splitcast-xtask -- verify-virtual` in the completed Terra review worktree with the private-audio test renamed. It exited 1 and reported `private-audio: FAIL`. `artifacts/verification/runs/1794601660992853669/private-audio.log` recorded zero executed tests and `Required opt-in test did not execute; refusing an empty pass.` Fmt also failed on then-unformatted verifier code; that is a separate result, not the evidence for the empty-test rejection. The renamed test was restored. V02 is closed by the per-check failure and the subsequent integrated all-green run.

`git diff --check` exited 0 after integration. The final ordinary verifier and independent integrated lifecycle verdicts are still pending at this checkpoint.

The library signature changes from `agent::serve(Config)` to `agent::serve(Config, Arc<AtomicBool>)`. The only repository caller is migrated. Crates remain unpublished application internals. CLI commands and successful HTTP response schemas remain unchanged. Error bodies add machine-readable outcome fields. No dependency, Cargo lockfile, Nix runtime gate, or autostart target changed.

Ordinary Stop is still queued behind active Prepare. The lifecycle test observes that restriction before requesting per-instance shutdown. In-progress blocking calls may wait for their current deadlines. Mandatory joins do not establish a universal hard shutdown bound, and process teardown still has per-worker grace and reap limits. Actual agent RTMP and RTSP media integration, congestion recovery, frame freshness, and all physical gates remain open software or hardware work as appropriate.

## Final ordinary verifier and independent runtime review

`nix develop -c cargo run -p splitcast-xtask -- verify` exited 0 with all 11 steps passing in `artifacts/verification/runs/1794621763416400929/`. It tested the same integrated production source as the 14-step run above. A later test-only graft from the independent shutdown probe requires its own final matrix entry below.

Terra independently ran the actual two-agent test, all 19 control integration tests, and the generated transport test successfully in its integrated snapshot. It also ran `nix develop -c cargo test -p splitcast-rs-core --test review_shutdown_probe review_sigterm_running_capture_cleans_both_agents --locked --offline -- --nocapture`. That probe sent SIGTERM while capture and production were Running, then observed production Idle and all seven helper PIDs and service ports released. No runtime-proven defect survived its scoped review. It did not run the complete verifier or accept hardware behavior.

The shutdown probe was retained as `sigterm_running_capture_releases_both_agents_workers` in the primary two-agent test target. Fixture directories now use a per-process sequence number so the two ordinary tests do not share state. No production source changed for this graft. Final matrix results and the other independent verdicts remain pending at this checkpoint.

The second comment audit retained three platform-contract comments as HOLDs, the two signal-handler SAFETY explanations and the unreaped Child identity explanation. No comments were deleted, no suppressions were added, and no unsupported safety claim was inferred from that comment-only pass.

## Final integrated result

`nix develop -c cargo run -p splitcast-xtask -- verify-virtual` exited 0 after retaining the running-SIGTERM test. All 14 steps passed in `artifacts/verification/runs/1794622014178620022/`. The ordinary suite passed 124 tests, including both two-agent scenarios. Three separate local opt-in checks passed. The [command summary](virtual-system-20261114-summary.tsv) is byte-checked against the actual run summary. The [source manifest](virtual-system-20261114-source-sha256.tsv) identifies 49 source and configuration files from the tested dirty tree. The earlier 123-test manifest remains unchanged.

Sol's independent integrated lifecycle review passed all 11 ordinary verifier steps and 11 lifecycle tests after adding six review-only adversarial probes. The probes exercised 256 execution-abandon settings races, 256 expired entries, 256 finish-abandon races, eight-slot listener teardown, cleanup despite caller cancellation, and an isolated descriptor-exhaustion accept failure. The original report claimed no forbidden settings writes after `not_executed`, but three file-observation assertions used the wrong directory and do not support that claim. V03 below supersedes those observations. The injected EMFILE propagated shutdown and released the listener. A review-only argument-type compile error was corrected and retained as a probe failure, not a product defect. The original report and logs are retained in `artifacts/verification/review/sol-original/`; the corrected report and instrument are in `artifacts/verification/review/sol-corrected/`.

Luna's independent integrated review passed the core tests, the delayed-Prepare shutdown test, and a complete 11-step ordinary verifier rerun. Luna reported that the first invocation hit its review tool's 120-second timeout after feature checks. The retained partial run is `artifacts/verification/review/luna/runs/1794621867847578012/`; it has no final summary. The longer rerun passed in `artifacts/verification/review/luna/runs/1794621991669756972/`. The tool-timeout status is reviewer-reported; the incomplete and complete artifact directories were inspected by the coordinator. The incomplete run remains distinct from a product failure or a full pass. Luna did not separately run the ignored local components or a live CLI SIGTERM probe.

All three integrated reviewers returned no verified product defects in their scoped snapshots. This return status does not validate Sol's three initially incorrect settings-file observations. V03 records their independent correction. Terra's running-SIGTERM probe was subsequently retained as the additional primary test described above. The final primary matrix reran that test successfully. No reviewer is credited with inspecting later documentation edits or the final test-only fixture-directory change.

This closes the run's scoped completion predicate, not full SplitCast completion. Actual agent media-path integration, congestion behavior, immediate Stop preemption, hard aggregate shutdown guarantees, durable command outcomes, browser ownership, settings fault injection, and physical acceptance remain open. No package deployment, Nix activation, commit, push, or PR was performed.

Rollback is a source-only operation. Build `aa86fe0d443d55a1a66c61198811a5b906ebef57` in another checkout if a comparison is needed. Preserve these evidence files and logs. Do not reset unrelated work or infer authorization to restore an operating-system generation.

## V03 correction to review instrumentation

During final reconciliation, the coordinating session found that two review-only Sol probes observed `fixture.0.state_dir/settings.json`. `Agent::new` appends the role directory, so the actual file is `agent.cfg.state_dir/settings.json`. The earlier review's claim about observing no settings-file writes in those two probes is not accepted on that instrument. Its other socket, ticket, and process observations retain their narrower scopes.

The two paths were corrected in the review worktree, and a fresh Terra reviewer is validating the corrected probes with a positive file-write control and a deliberately broken gate. The primary product tests already use `agent.cfg.state_dir` and previously failed on a real forbidden write under mutation. No primary production source changed for V03. Preserve the original probe identity and report, and append the corrected command outputs before accepting the additional race evidence.

The fresh reviewer found a third wrong-path assertion in the listener-drop probe. All three settings-file observations in the original report are superseded by `artifacts/verification/review/sol-corrected/report.md`. The original instrument is preserved byte-for-byte as `original-instrument-invalid.rs.txt`, SHA-256 `29d5167af534cab15079466742d496f048641bacb319135555a1376a1b277abc`. The final corrected instrument is `final-probe.rs.txt`, SHA-256 `566c5ffe628178938105122d3449ea514448ab28b3d3b6c484c395056f64d3e2`.

The positive control executed a valid settings change and read `audio.mic_offset_ms=123` from the actual production state file. `nix develop -c cargo test -p splitcast-rs-core --lib agent::lifecycle_tests::adversarial_abandoned_work_does_not_write_settings --locked --offline -- --nocapture` then failed under a deliberate two-part gate mutation because an abandoned request wrote that exact file. The product source was restored byte-for-byte. The corrected complete lifecycle suite passed 13 tests. In its 256 settings races, 128 responses were `not_executed`, 128 were `unknown`, and none were completed. File bytes were unchanged after each `not_executed` outcome. Expired entries and listener-drop requests left the actual file absent. These observations cover persistent settings changes in those samples, not exhaustive schedules or every possible transient effect.

The corrected-review verifier initially failed Clippy on a needless cast in the review probe. Removing that review-only cast produced a complete 11-step pass in `artifacts/verification/review/sol/runs/1794622583292664594/`. The earlier Clippy failure remains in the adjacent `1794622493535257531/` directory. No primary product source changed. V03 is closed as an instrumentation correction, not a product bug fix.

## Final artifact and documentation checks

The tested native debug executable has SHA-256 `e24b1d46d355830575d2fdc97d9444777a864d92bc29acbc83e1882cf51abc40`. The verifier executable has SHA-256 `ed743a44e3ff5c62044aae13752a2c337b930c1311d53c0c4983608a8d50ada6`. The local record is `artifacts/verification/runs/1794622014178620022/artifact-sha256.tsv`. These are debug artifacts used by local tests, not Nix deployment packages.

After the documentation updates, `nix develop -c cargo run -p splitcast-xtask -- portability` exited 0 and `git diff --check` exited 0. A local Python check resolved all 42 relative Markdown links in the then-changed current documents and byte-compared the published command summary with the final raw summary. The summaries were identical. This was a local artifact check, not an application behavior test.

## Retained review and trail audit pointers

The original design sketches and cross-judgment are copied to `artifacts/verification/review/design/`. The initial missing-target command and failure log are copied to `artifacts/verification/review/initial-missing-target/`. Their temporary originals remain untouched. The Sol original verifier is retained at `artifacts/verification/review/sol/runs/1794621865431397595/`. Terra's shutdown probe source is retained at `artifacts/verification/review/terra-shutdown-probe.rs.txt`, with its two-agent fixture state under `terra-fixtures/`. Terra's command outcomes were received as its review report; those fixture directories are not represented as full command-output logs.

The cross-model trail audit checked the 14-step summary, 124-test total, all three nonempty opt-in results, all 49 source hashes, and the primary forbidden-settings mutation. It flagged V03, unclear review-artifact pointers, a stale pending statement in the design document, and weak temporary-location retention. V03 was independently corrected as above, the pointers and current design statement were corrected, and the temporary artifacts were copied without changing their originals. The audit could not inspect the full conversation because this runtime exposes no transcript resource. It is an artifact and record audit, not a transcript audit.
