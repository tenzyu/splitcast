# Real production-media evidence

Recorded 2026-11-15 JST. Run-directory timestamps use UTC on 2026-11-14 and 2026-11-15. S20/E16 covers the generated production-only profile, not physical capture, capture-Agent publication, SSH, deployment, or a performance benchmark.

## Timing correction AV01

The initial `0accf63` runs below passed their then-current assertions, but their phase-fit A/V claim is superseded. A pure probe demonstrated that 800 ms of delay could be accepted as a 500 ms fit. Review exposed repeated windows in an intermediate audio code, hidden audio PTS gaps, and unchecked opening audio identity. Correction `67720ca71c6f8026854024b56a9e4d2ee403370a` uses unique source-second frequencies, cumulative sample/PTS validation, explicit transition uncertainty, and identity checks including both boundaries. It does not promise continuous per-sample synchronization.

Targeted command `nix develop --no-write-lock-file .#media-test -c bash tools/verify-media.sh --case normal` passes ten pure tests and the real normal case in `20261115T021238Z-npZX2Fe0`. All three modified OBS recordings were generated and rejected for the intended A/V source-identity or PTS-gap errors, not a codec or fixture failure. The earlier `20261114T222049Z-oGQBu8lX` normal case failed the boundary-mutation encoder's unspecified channel layout. The committed stereo `aformat` correction fixes that fixture. A selected-case command still exits 1 because it is not the complete matrix.

Full run `20261115T021352Z-1bP8vVaC` at `67720ca` failed normal and passed the other five cases. Normal expected a video transition at 11.850 seconds, after its last complete audio analysis window. Cleanup passed. The focused correction now checks actual stream start/end skew against the unchanged 500 ms envelope and records partial boundary video markers as unmeasured. It still requires three jointly observable transitions and source-identity compatibility in every audio window. The failed run is retained; its normal case is not counted as a pass.

## Corrected final-source evidence

Tested implementation is `42a64e41990eb3a411dcd68e3412ccceac574112`, including `67720ca` and the focused EOF correction. Media source-manifest SHA-256 is `60987062daaf94d2a9fb57308edfe8756a4e54303f813eac53e2c22f2571e54b`. Build and after-run source identities agree. Rust/application, xtask, Nix and lockfile contents are unchanged from `0accf63`; the controller binary and pinned environment identities below therefore remain applicable. Later evidence/memory-only commits are not new runtime executions.

| Exact command | Actual result and private receipt |
| --- | --- |
| `nix develop --no-write-lock-file .#media-test -c bash tools/verify-media.sh --case normal` | Eleven pure tests and normal PASS, `20261115T022438Z-JpHZEiyd`; exit 1 because this is a subset. |
| `nix develop --no-write-lock-file .#media-test -c cargo run -p splitcast-xtask -- verify-media` | Exit 0, eleven pure tests and all six cases PASS at `42a64e4`, `artifacts/media-plane/runs/20261115T022547Z-vrJRNZQD/`. Same-controller recovery, unchanged source, and all cleanup checks pass. |
| `nix develop --no-write-lock-file -c cargo run -p splitcast-xtask -- verify-virtual` | Exit 1 at `42a64e4`, `artifacts/verification/runs/1794709924594226389/`. Thirteen stages pass; no-default-features fails on `late_owned_start_cannot_recreate_a_completed_remote_session`: initial capture Prepare resets the connection before the replay assertion. No cause established; no speculative code change. Retry result recorded below. |
| `nix develop --no-write-lock-file -c cargo run -p splitcast-xtask -- verify-virtual` | Single retry exits 0 at unchanged `42a64e4`, all 14 stages PASS, `artifacts/verification/runs/1794710203786488803/`. Both default and no-default-feature suites pass 155 ordinary tests each; three local components run separately with one test each. Zero doctests are not example coverage. The earlier reset remains an unresolved observation, not a fixed bug or proven scheduling flake. |
| `nix develop --no-write-lock-file -c cargo run -p splitcast-xtask -- verify` | Exit 0 at unchanged `42a64e4`, all 11 stages PASS, `artifacts/verification/runs/1794710355626646567/`, including default and no-default-feature behavior. |

The final normal MKV is H.264/AAC, 1280×720 at 30 fps, 48 kHz stereo, 11.900 seconds with 354 decoded frames and 11.7 seconds of source-content progression. Scene ages are 1.963–2.007 seconds. Both audio identities pass. The largest accepted sampled game A/V transition upper bound across the matrix and recovery is 0.271 seconds including uncertainty, inside the 0.500-second envelope. This is not a continuous per-sample, microphone, cross-host or physical timing proof.

Frozen game bytes advance from 129,383 to 264,516 while scene age reaches 14.481 seconds and the independent content check rejects the recording. The bounded four-second source backlog advances bytes from 164,615 to 351,322 while age is 5.925–5.974 seconds. Its 120-frame queue peaks at 111,513,600 bytes; it is not network congestion. Disconnect, publisher stall and RNNoise death are detected in 2.992, 4.047 and 2.198 seconds respectively. Explicit recovery on the same controller records epoch 15, 11.866 seconds and 356 frames. All cleanup-error lists are empty; no cleanup retry was needed.

Three mutations of the final actual OBS recording were successfully generated and rejected:

| Control | Intended observed rejection | Mutated-file SHA-256 |
| --- | --- | --- |
| 800 ms stream offset | `A/V stream endpoints exceed envelope` | `d8ccc01443c4b0b6e2227261b45080c340f4e33f95c451924efe1917d2603111` |
| Internal 800 ms audio PTS gap | `A/V audio PTS discontinuity: 0.8003333333333336` | `8984139f8a94f43bc5351c2622f81d434f3ae4fbdd25fb0e7104680fcab3ecc9` |
| Incorrect opening audio | `A/V audio source identity has no nearby video observation` | `eb71f5ba21f32d43c2ecc059c83afd6ba16780bddd6a5cac780d2a7b31668e5c` |

Final normal output SHA-256 is `66c7e4209b9fb24c041667d6d31297a56999418300eccf6c57e0b1b36b13559f`; recovery is `4c11c499f877013a3c0c0e4e3f641890c67f07bbfe77ba0e6c2dfdecb15f1545`. Raw media, process receipts and logs remain private.

## Preserved pre-AV01 source and environment

- Implementation commit `0accf63dd40a81142f19f56f2f765a1ede23ffa2`, based on the actual clean `main` at `2a774d925bafd008a0dedef6a8fd0050043d89b6`. Main already contains control-plane merge `6349fd6`. No other branch was merged, rebased, reset, or copied into this work.
- The `0accf63` media source-manifest SHA-256 is `a4bdafa5f108a513e1a1a57c7c0242cdac1550a8896416b91fc02e0626cd1705`. This is preserved historical provenance, not the corrected oracle's source hash. All non-documentation source inputs, including the OBS patch and launcher, are hashed.
- Default-feature Cargo dev binary SHA-256 `e59bd337bb4506268ad4b7c0d8915b9637889f3865dbd201422524d310422af1`. Build command `cargo build -p splitcast-rs-core --locked --offline`. This is not an installed NixOS generation or a release-performance result.
- `Cargo.lock` SHA-256 `148f1d305385c8ad8c94eef8a3d652688d6aaf8e29405aa48552b90bafe8ff9d`. `flake.lock` SHA-256 `27c4f4206ffd635aaea7cd0ab6c3c7bf611c5603b0ba72dd2787ef7dfefea220`.
- Pinned `x86_64-linux` media shell. Rust 1.95.0, OBS 32.1.2 with the existing loopback patch, MediaMTX 1.18.2, FFmpeg 8.1.2, PulseAudio 17.0, RNNoise LADSPA 1.10, Xorg/Xvfb 21.1.24, Bubblewrap 0.11.2, and Python 3.13.15. Raw executable hashes and the full OBS Nix closure remain private. Closure-list SHA-256 `be96d04d6f1e4925dcbd924e9446efdf8e7d33db34b5db1ecaf7bd39abefe712`.

Source checkout, built controller, generated source epochs, running workers, and recorded outputs have separate identities. Every service under acceptance is real. Only game and microphone ingress are synthetic. The Agent owns MediaMTX, private PulseAudio, RNNoise, and OBS. The profile uses authenticated production Prepare, Start, Stop, and fresh recovery commands.

## Preserved pre-AV01 commands and results

| Exact command | Result and private receipt |
| --- | --- |
| `nix develop --no-write-lock-file -c cargo run -p splitcast-xtask -- verify` | PASS, all 11 checks at `0accf63`. `artifacts/verification/runs/1794692996286907638/`. Earlier passing run `1794692233718508169` retains its precommit identity. |
| `nix develop --no-write-lock-file -c cargo run -p splitcast-xtask -- verify-virtual` | PASS, all 14 checks at `0accf63`. `artifacts/verification/runs/1794692450019307764/`. Includes 155 ordinary tests, default and no-default-feature tests, valid all-features check, WASM, fmt, Clippy, dependency trees, portability, and three separately executed local components. Zero doctests are not example coverage. |
| `nix develop --no-write-lock-file .#media-test -c cargo run -p splitcast-xtask -- verify-media` | PASS at `0accf63`. `artifacts/media-plane/runs/20261114T214316Z-ePKoNxTB/`. Five pure oracle tests and all six real-media cases execute. The same-controller recovery is part of worker-death. The full matrix and unchanged-source gates are true. |

The [profile](../media-acceptance.md) and `LIMITS` predeclare the assertions. Normal recording has a ten-second observation window, with encoder startup allowed inside a 9–13 second file envelope. Fault files allow the additional 20-second detection budget. Their content checks cover the pre-fault ten-second prefix, while packet checks and full-file decode still cover the entire file.

| Pre-AV01 case | Observed result |
| --- | --- |
| Normal | H.264, AAC, 1280×720, 30 fps, 48 kHz stereo. 11.833 seconds, 354 decoded video frames, 11.8 seconds of content progression. Both game and processed microphone identities pass. Ten local scene-age observations range from 1.962 to 1.980 seconds. |
| Frozen video | Game bytes increase at every sample, from 128,996 to 263,424. Scene age ranges from 5.479 to 14.447 seconds. The recording oracle rejects `frozen content progression gap`. Agent liveness still admits stale content. |
| Generated backlog | A 120-frame queue before RTMP publication peaks at 111,513,600 bytes, below 128 MiB. Game bytes increase from 163,588 to 350,398. Scene age ranges from 5.909 to 5.943 seconds and fails the three-second independent age gate. The 11.833-second recording fully decodes with progressing content. This is not delayed network packets. |
| Disconnect | Actual game publisher termination leaves Running, observed after 3.010 seconds. The finalized 12.666-second file fully decodes. Pre-fault content and both audio identities pass. |
| Stall | Actual game publisher SIGSTOP leaves Running, observed after 4.101 seconds. The finalized 13.800-second file fully decodes. Pre-fault content and audio pass. The source receives SIGCONT before teardown. |
| Worker death and recovery | The identity-checked RNNoise worker receives SIGKILL. Failure is observed after 2.192 seconds. After completed Stop, the same controller accepts explicit Prepare and Start and records new epoch 15. Recovery is 11.766 seconds with 353 decoded video frames and a distinct output hash. Ten recovery scene samples identify the new epoch. |

The old phase-fit estimator reported zero offset. AV01 disproved the adequacy of that estimator, so this number is not retained as synchronization proof. The source-generation and screenshot timestamps still provide separately valid local scene-age observations on the same monotonic clock.

Every `0accf63` case reports empty cleanup errors. All owned process identities disappear, reserved TCP ports refuse connections, source feeder threads join, and the private Pulse socket is absent. No explicit cleanup retry was needed in that run. Generated artifacts and logs remain private. Controller SIGKILL and escaped-process deployment containment were not accepted by this run.

Normal output SHA-256 is `bbc7f3a1bbf093fa2d81efbcdfdd676a7ca675be5902bf4c4cb00426f17ef8c4`. Recovery output SHA-256 is `af20ca8a430f47e78c3a46849ccbf1616f2c624779d6e4fe2e88d5b28d4565d3`. Raw probes, packets, PCM analysis, screenshots, source ledgers, command responses, tool identities, and cleanup receipts remain private and reproducible through the profile.

## Failed evidence and corrections

Earlier results are not promoted to this final source. Every private run remains under `artifacts/media-plane/runs/`. The canonical decision trail is `.audit/media-plane.tsv`.

- Baseline `nix develop --no-write-lock-file -c cargo run -p splitcast-xtask -- verify-virtual` failed portability in run `1794689069939760450`, while its other 13 checks passed. Ignored `.direnv` symlinks were incorrectly scanned as source. `nix develop --no-write-lock-file -c cargo test -p splitcast-xtask direnv_cache_is_not_runnable_repository_source --locked --offline -- --exact tests::direnv_cache_is_not_runnable_repository_source` reproduced `unexpected repository symlink .direnv/flake-profile` before the correction. All four xtask tests then passed.
- The first `nix develop --no-write-lock-file .#media-test -c python3 tools/media_acceptance.py --case normal` attempt, `20261114T205659Z-3bd9741c`, opened a GPU render node through OBS enumeration despite software selection. It was rejected. No capture input was configured. Subsequent execution requires a device-free mount namespace, private PID and network namespaces, read-only source, and a writable fresh evidence directory. Do not describe the initial attempt as device-free.
- Diagnostic command `nix develop --no-write-lock-file .#media-test -c bash tools/verify-media.sh --case normal` produced distinct retained failures. `205856Z-d8c40bc8` and `205936Z-9d92a0fe` failed XKB startup. A private strace probe showed readable keyboard data but `execve("/bin/sh")` returning ENOENT. `210135Z-a0d7f0f9` then identified missing EGL vendor configuration. `210322Z-430485cd` identified an unescaped FFmpeg filter comma. `210428Z-d67a9f24` identified Pulse's non-dumpable process denying FD inspection. The final namespace structurally hides devices and inspects all listeners, while recording denied optional FD observations honestly.
- The same diagnostic command exposed an early OBS scene-load rejection in `210609Z-tVSH1eLo`, RNNoise's RTSP microphone 404 in `211007Z-KngPu4hi`, and a first-health progress failure in `211046Z-U2dtdNqe`. `211153Z-uR0gJ1wN` and `211424Z-jIQkjqNc` exposed black preview content. The generated initial scene now exists, preparation waits for it, RNNoise waits for published tracks, and Start waits for source geometry and actual output bytes. The first-health error did not identify its exact counter cause, so it is not a proven encoder-only diagnosis.
- A short stopped output failed independent EBML parsing. `nix develop --no-write-lock-file -c cargo test -p splitcast-rs-core stop_waits_for_recording_finalization_before_returning --locked --offline` failed before the fix with `GetStreamStatus` where `GetRecordStatus` was required. Stop now waits for inactivity under one deadline. Unconfirmed finalization propagates through Agent cleanup, which still attempts every worker. The retained-cleanup regression verifies Stopping rather than false completion, followed by explicit retry.
- The first full `verify-media` run, `20261114T211833Z-O7IfzTqm`, failed four cases. Normal and stall passed. Frozen Stop retained an owned OBS obligation. The RTMP relay delayed control exchanges and timed out rather than proving media delay. Long private Pulse socket paths broke disconnect startup and worker-death recovery. The final profile uses the short `/evidence` mount alias and a bounded generated-frame backlog. Network-delay acceptance remains open.
- Independent review falsified a total-span-only continuity check with a three-second frozen tail. Pure tests now reject partial freezes at the beginning, middle, and end. Review also removed arbitrary binary overrides and writable source mounts, expanded source hashing to patch inputs, and added the OBS closure receipt. Run `20261114T212944Z-vxz8FF3b` predates the FPS and audio-geometry assertions. Run `214316Z-ePKoNxTB` passed those assertions but predates AV01. Neither is final corrected-oracle proof.
- A diagnostic subset returns exit 1 even when its selected case passes. This prevents a normal-only run from becoming complete acceptance. A zero-test run is never evidence. The parser delegate also reported one wrong-package command using `-p splitcast-rs`; that setup error was not behavioral red evidence.

Python stdlib orchestration and one typed video mode were selected over more elaborate orchestration or profile-rewriting wrappers. Prior reviews supplied the recorded counterexamples. The final boundary-fix reviewer hit a provider usage limit and returned no verdict. Human review remains pending; no approval or merge-ready status is claimed. The trail's first timestamp is corrected as a task-date placeholder, not an observed UTC start time.

## Still open and rollback

P1 is narrowed, not closed. Capture-Agent publication and SSH, actual network congestion, continuous live freshness enforcement, microphone A/V offset, default-VAD speech quality, long-run drift, camera and speech features, physical GSR, GPU contention, gameplay, deployment ingress, and forced-controller-death containment remain unaccepted. Full Nix package and consumer rebuilds, advisory scans, Miri, fuzzing, other architectures, and physical tests were not rerun here. No old pass is substituted for those checks.

Next evidence-generating work is a generated-source capture-Agent and local SSH topology, a media-aware network impairment experiment that does not stall the RTMP handshake, and an independently timestamped microphone marker at the intended VAD setting. None requires weakening this profile or authorizes physical capture. Live health must retain the distinction between transport progress and fresh decoded content.

Rollback means building the prior `2a774d9` source in a separate checkout or reverting the candidate in a reviewed change. Do not reset concurrent work, restore an OS generation, or activate a fallback. `rustCandidate=false` and empty autostart targets remain unchanged.
