# Verification — source-bound evidence, not a global PASS

**Historical public main checkpoint:** `ed7fe60e8efd0dea454346b13f38a680096fb60e`. GitHub [run 44097472963](https://github.com/tenzyu/splitcast/actions/runs/44097472963) completed with **four successful jobs**: `verify`, `verify-virtual`, `Native and WASM packages`, and `Delivery scripts`. No live Twitch/game/microphone/output/playback or deployment is covered by that result. [Original verification transcript](https://github.com/tenzyu/splitcast/blob/archive/pre-dreaming-20261117/docs/verification.md) preserved by source ref.

The [audio return receipt](evidence/audio-return.md) records the two-Agent software result, failure injections and independent private-sink observations on the audio return candidate.

## 2026-11-18 convergence harness integration

| Exact command or profile | Observed result |
| --- | --- |
| `sha256sum -c MANIFEST.sha256 && python3 -m unittest tools.convergence.test_prototype -v` | Attached prototype manifest verified. 29 tests passed in 17.029 s in an extracted temporary directory. This evidence covers the prototype. |
| `direnv exec . cargo run -p splitcast-xtask -- verify` | Initial local integration passed all 10 checks. Receipt `artifacts/verification/runs/1794981111567417251/`, with HEAD `8c54a4ecaecaa4e6c292b59886ff6405caf50d15` and worktree changes listed in the receipt. |
| `direnv exec . python3 -m unittest discover -s tools/ci -p 'test_*.py'` | Initial integration ran 31 tests with one Git-cache protocol test skipped by its opt-in gate. |
| `nix shell --inputs-from . nixpkgs#actionlint nixpkgs#shellcheck nixpkgs#python3Packages.pyflakes -c bash tools/ci/lint.sh` | Initial integration passed workflow, shell, Python and documentation checks. The signed-cache tamper control failed to import deliberately corrupted fixture data as expected. The protocol suite passed all 10 tests. |
| [Main CI](https://github.com/tenzyu/splitcast/actions/runs/44245497263) | Delivery scripts, verify, verify-virtual and Native package all passed on the integration base `8c54a4ecaecaa4e6c292b59886ff6405caf50d15`. |

Three existing work units were replayed twice directly and then through the
harness on the same allowlisted dirty checkout at `8c54a4e`, snapshot
`588ad697353322b6c552b4aa60a903de04b7e7d4294155adc5b1e762f6401ec0`.
All three met their existing software acceptance criteria both ways. Each
replay compared two direct invocations with four harness invocations
(enroll, run, reuse, status). Actual check launches fell from two to one per
unit. The comparisons used one workstation and a warm Nix store.

| Existing check argv | Direct total | Harness total | Reuse invocation |
| --- | ---: | ---: | ---: |
| `python3 tools/ci/check_docs.py` | 0.186 s | 1.420 s | 0.135 s |
| `python3 tools/ci/test_finalize_release.py` | 0.288 s | 1.493 s | 0.138 s |
| `env SPLITCAST_CACHE_PROTOCOL_TEST=1 python3 tools/ci/test_release_cache.py` | 13.239 s | 6.531 s | 0.139 s |

Private summaries in `artifacts/convergence-replay.json` name the source,
receipt and sanitized log digest per case. Receipt reuse reduced repeated
execution. Two short checks cost more with enrollment and status included,
so retain the direct workflow for short one-off checks. These local replays
included zero review rounds. Token counts and longitudinal review churn were
unmeasured. Physical acceptance and live Codex were outside this run.

## 2026-11-18 additional PR #16 review

| Exact command or profile | Observed result |
| --- | --- |
| `direnv exec . cargo test -p splitcast-rs-core --locked --offline --test two_agents -- production_watchdog_ tui_quit_ tui_eof_ --nocapture` | On `e8d9427` with the new fixtures, the watchdog kept a missing-frame Twitch stream Running. Both terminal cases lacked the expected notices. Three regressions failed. Private log `artifacts/mission/pr16-managed/review-f13-f14-red.log`. |
| `direnv exec . cargo test -p splitcast-rs-core --locked --offline --test two_agents -- production_watchdog_ tui_quit_ tui_eof_ tui_can_stop_ --nocapture` | Four passed on the source committed as `eb0d4ac`. The authenticated production Agent accepted advancing counters and stopped on missing, wrong-type, unchanged and decreasing stream frame counters. It confirmed output inactivity, and the fixture observed released workers and listeners. The terminal tests held the Start response and checked Quit, EOF and urgent Stop. An intermediate run retained a failed fixture assertion that used the wrong cleanup field name. Private log `artifacts/mission/pr16-managed/review-f13-f14-focused-final.log`. |
| `direnv exec . cargo run -p splitcast-xtask -- verify-virtual` | All 14 checks passed on `eb0d4acd6369cadacb22aee7bd0cc159a69ca754`. Receipt `artifacts/verification/runs/1794976855239153906`. |
| Hosted CI on `e8d9427` before F13 and F14 | Four checks passed, including Native package. The package receipt binds the GitHub test merge `205a44e`. [Hosted run](https://github.com/tenzyu/splitcast/actions/runs/44236505311) |
| Hosted CI on `abf6f3d` with F13 and F14 | All four checks passed. [Hosted run](https://github.com/tenzyu/splitcast/actions/runs/44239360350) |
| `nix develop --no-write-lock-file --max-jobs 1 --cores 4 .#media-test -c bash tools/verify-media.sh --case normal` | On `abf6f3d`, packaged OBS recorded and decoded to EOF, confirmed inactive output, exited with code 0, and released its process and listener. The negative control failed. One notch left the 180 Hz mic signature in enough windows to keep the longest detected dropout at two. All 117 game windows retained their identity. Receipt `20261118T045553Z-SmjVkppW`. |
| `nix develop --no-write-lock-file --max-jobs 1 --cores 4 .#media-test -c cargo run -p splitcast-xtask -- verify-media` | All seven device-free packaged-OBS cases and the 16 media oracle tests passed on `1f7f8649c7144164a4d706995b96484ecca83672`. On the retained failed sample, the replacement high-pass mute held the game identity in all 117 windows and removed the mic signature for 14 consecutive windows. The fresh normal case rejected the mutation at 14 consecutive windows and verified inactive output, OBS exit code 0, and resource cleanup. Receipt `20261118T045941Z-T1dNJYup`. |

The review follow-up used isolated OBS WebSocket responses for counter faults.
All 22 public and 62 private samples from the earlier Twitch sessions supplied
strictly increasing frame and byte counters. The physical observations below
remain bound to the earlier executable. The production VM recovery and prior
display Stop failures have a [separate host receipt](evidence/pr16-managed-acceptance.md#failures-and-cleanup).

## 2026-11-18 managed broadcast follow-up on PR #16

The [integrated physical and viewer receipts](evidence/pr16-managed-acceptance.md)
identify the source, binaries, observed output, failed starts, host residue and
remaining acceptance gates. The [controlled OBS comparison](evidence/pr16-obs.md)
covers selected generated media cases and preserves the reproduced SIGSEGV.

| Exact command or profile | Observed result |
| --- | --- |
| `direnv exec . cargo run -p splitcast-xtask -- verify` | PASS on `4e3bffe1eac876e426d3805a7c29b21f6b2b7743`, run `1794973440194326628`. Earlier attempts retained the Clippy revocation match failure, an intermediate TUI output-lock compile failure, virtual Prepare timeouts, and the outdated redirect fixture assertion before it was updated to verify the new Status/Start sequence and authentication. |
| `direnv exec . cargo run -p splitcast-xtask -- verify-virtual` | PASS on `d9b0948435877039ba3b55dc4797566ef73d9fec`, run `1794974733695648133`. This includes the ordinary Rust checks, two actual Agents, private Pulse, generated media, native return and local transport. |
| `direnv exec . cargo test -p splitcast-rs-core --test two_agents chromium_speaks_through_the_agent_and_observes_stop --locked --offline -- --ignored --nocapture` | PASS at `d9b0948` with pinned Node and Chromium provided through `SPLITCAST_TEST_NODE` and `SPLITCAST_TEST_CHROMIUM`. The actual browser sent authenticated commands to the served Agent. Private receipt `artifacts/mission/pr16-managed/chromium-final.log`. |
| `nix develop --no-write-lock-file .#media-test -c bash tools/verify-media.sh --case normal --case camera` | Both selected packaged-OBS cases passed, including independent output decode, exact process/listener observation and normal exit. The partial-profile command returned 1 by design. Run `20261118T032417Z-KZVOhzn5` records the earlier tracked-source manifest. |
| `nix build --no-write-lock-file --max-jobs 1 --cores 4 --no-link --print-out-paths .#default .#checks.x86_64-linux.configs` | PASS on `4e3bffe`. The preceding build failed at the CLI redirect fixture because it expected Start before Status. The derivations also validated both role examples. |
| Same Nix build command on `d9b0948` | No local package result. Two attempts stalled in the local Nix daemon. The second worker's kernel stack showed `ptmx_open`. The owned build client was terminated. The shared daemon was left in place. Private receipts `nix-final.log`, `nix-final-attempt-2.log` and `nix-daemon-kernel-605344.txt` under `artifacts/mission/pr16-managed/`. |
| Hosted `verify-virtual` on `61e5077` | Attempt 1 failed before invoking the verifier. Nix failed to resume downloading the GStreamer substitute from `cache.nixos.org` (HTTP 416). [Hosted run](https://github.com/tenzyu/splitcast/actions/runs/44235744447/attempts/1) |
| Source-bound physical private recording and public Twitch tests | PASS for integrated composited media, decoded viewer playback, private/public Stop and actual OBS exit at `d9b0948`. Physical A/V alignment, p99 gameplay timing, production late-render target and complete Xorg rollback remain open. The public game trial observed chat admission, with synthesis and capture-host playback unconfirmed before Stop. |
| Source-bound private Twitch bandwidth test | PASS at `d9b0948`, generated media on production. Final observation span 302.036 s across 62 samples. Verified Stop, exit code 0 and empty cleanup error list. Initial private setup attempts failed with a wrong systemd working directory and missing media-shell environment; both are preserved. |

Specialized Miri, fuzzing and live production forced-controller-death were not
run in this follow-up. The recordings were bounded to trial storage, and the
independent decoder retained only sanitized receipts after cleanup.

## 2026-11-18 native PoC candidate

The live trial and independent observations are summarized in the
[versioned Wiki](memory/session-state.md). The original PR #15 speech receipt
remains unchanged.

| Exact command or profile | Observed result |
| --- | --- |
| `nix develop -c cargo run -p splitcast-xtask -- verify` | PASS on `17594cf5c1116e2bb6c37917850446c7d1081897`. Receipt `artifacts/verification/runs/1794966619556727355/`. |
| `nix develop -c cargo run -p splitcast-xtask -- verify-virtual` | First isolated run failed because the fixture still used the removed mutable routes command. The final run passed all 14 checks, including real private OpenSSH/PipeWire. Receipt `artifacts/verification/runs/1794966010817819864/`. |
| `nix develop .#media-test -c bash tools/verify-media.sh --case normal` | Initial normal case failed the mic dropout oracle with five consecutive windows. It passed after Start began waiting for the owned RNNoise stream on the private sink. The diagnostic subset leaves the aggregate result INCOMPLETE by design. Receipts `20261118T012927Z-zIVNZMBr` and `20261118T014007Z-kpinXt33` under `artifacts/media-plane/runs/`. |
| `nix develop .#media-test -c cargo run -p splitcast-xtask -- verify-media` | The first full matrix rejected the mic-dropout mutation at the overall identity floor, instead of the consecutive dropout oracle. The mutation now lasts 1.5 s, above the continuity limit and below the identity floor. All six cases and 16 oracle tests passed on the final source. Receipt `20261118T015020Z-P2gGgxKO`. |
| `nix build --no-link --print-out-paths --no-write-lock-file --max-jobs 2 --cores 4 .#default` | PASS after correcting the private Pulse sink fixture. Earlier package attempt failed seven two-Agent tests because that fixture reported the mic as a second private sink. |
| `nix build --no-link --print-out-paths --no-write-lock-file --max-jobs 2 --cores 4 .#checks.x86_64-linux.configs` | PASS. Packaged lifecycle selfcheck and both role configurations. |
| `nix eval --json --no-write-lock-file --override-input splitcast <candidate-flake> <target>` | With nixfiles `fc5ef27fd78dc1702681e07baf0775ce23eb75ca`, capture remained disabled. Production reported migration-gated ExecStart, empty WantedBy, and LimitCORE=0. The first production probe assumed ExecStart was a string. The corrected list-aware probe passed. |
| `python3 -m unittest discover -s tools/ci -p 'test_*.py'` | 12 PASS, 1 opt-in test skipped. The first run exposed a fixture variable lost when removing the WASM cache root. |
| `ffmpeg -nostdin -v error -xerror -i <trial-mkv> -map 0:v:0 -map 0:a:0 -f null -` | Exit 0 on the real two-host recording. Independent FFprobe frame/packet timestamps, decoded frame hashes and sample counts are retained under `artifacts/mission/`. The recording and derived preview were removed after inspection. |

The intermediate `17594cf` comparison counts physical lines in shipped Rust, native schema
and browser assets. It excludes test-only modules, fixtures, archives and
generated artifacts. Shipped code went from **7,375 to 6,408 lines** (−967,
13.1%) and from **26 to 21 source files**. The workspace went from four
packages to two. The Linux native crate tree grew from 57 to 70 resolved
package/version entries as the existing WebSocket and HTTP clients gained
certificate-verified TLS for Twitch. The built Nix runtime closure contains
438 paths and 2.22 GB of NAR data (including native media dependencies).
The baseline runtime output was absent locally, so no comparative closure
size is asserted. That checkpoint removed the TUI, browser
assets and crate, shared UI protocol crate, camera, public OBS publishing,
mutable route/settings persistence and legacy speech file/ack queue.
EventSub owns its WSS, identity validation, migration, bounded admission
and replay set. Authenticated control, session fencing, Stop reconciliation,
worker ownership, expiry and isolated playback remain product requirements.

## Restored PoC scope

Camera, Web, the terminal menu and Twitch publication were restored after the
intermediate native checkpoint above. On the same count, shipped source is
7,061 lines across 26 files. This adds 653 lines to `17594cf` and removes
314 relative to main (4.3%). The Web assets are embedded in the native Agent,
the TUI shares the CLI request path, and the build still has two workspace
packages and 70 Linux native dependency entries. The separate WASM client
and protocol crates, editable route graph and settings store are absent.

| Exact command or profile | Observed result |
| --- | --- |
| `nix develop -c cargo run -p splitcast-xtask -- verify` | PASS on the restored source, including fmt, check, Clippy, workspace and feature tests, dependency checks, and portability. Receipt `1794969551364775214`. |
| `nix develop .#media-test -c bash tools/verify-media.sh` | First restored matrix failed three cases on 4–8 consecutive missing windows from the mic. The first recording's gaps were at 0.0–0.3 s. The camera case independently decoded the expected inset. Receipt `20261118T021155Z-tpKWVRxX`. |
| `nix develop .#media-test -c cargo run -p splitcast-xtask -- verify-media` | PASS across seven cases and 16 oracle tests after bounding the Pulse playback buffer at 100 ms with a 20 ms prebuffer. The normal recording had 117 of 117 windows present for each audio identity, and steady-state video maximum PTS gap was 34 ms. Receipt `20261118T021940Z-3tX6ZcgE`. |
| Same media profile with the public output cap | PASS across seven cases and 16 oracle tests. Source tree `163507570d2fd62364ff89c655ce941336477c7233049827a18f7d4c73476961`, receipt `20261118T023211Z-Zwejiux7`. Binary SHA-256 matches the public Twitch trial. |
| `nix develop -c cargo run -p splitcast-xtask -- verify-virtual` | PASS on the restored source after authenticating readiness with a fixture-specific bearer. The first run hit a connection refusal after TCP-only readiness on capture. Failure receipt `1794968808086758260`. Passing receipt `1794969002443753154` includes fmt, check, Clippy, workspace, docs and feature tests, native return and local transport. |
| `nix build --no-link --print-out-paths --no-write-lock-file --max-jobs 1 --cores 2 .#checks.x86_64-linux.configs .#splitcast-web` | PASS for the native package and both role configurations. Web resolves to the same embedded-asset package. The Nix runtime closure remains 438 paths and 2,222,799,096 NAR bytes. Receipt `artifacts/mission/nix-restored-public-final.log`. |
| `python3 -m unittest discover -s tools/ci -p 'test_*.py'` | 12 PASS and one opt-in skip. Receipt `artifacts/mission/delivery-restored-final.log`. |
| Chromium 151, native Asset and control listener | PASS for loading, same-origin authenticated status, completed Stop and empty browser storage. Separate Node logic run checked stale Start and urgent Stop ordering. Receipts `artifacts/mission/web-browser-attempt-1.log` and `web-js-receipt.log`. |
| Physical camera, Twitch bandwidth check and public ingest | PASS within the respective slice boundaries. Source identities, cleanup and verification scope are in the [current acceptance record](memory/session-state.md). |

## Verification entry points

With direnv/Nix shell already entered, execute commands directly; otherwise use the `nix develop -c` wrapper.

| Contract | Command | Scope |
| --- | --- | --- |
| Source/unit/Clippy matrix | `cargo run -p splitcast-xtask -- verify` | No physical device/stream required |
| Virtual control and private opt-ins | `cargo run -p splitcast-xtask -- verify-virtual` | Actual local controllers and stand-in workers; not physical media |
| Generated production-only media | `cargo run -p splitcast-xtask -- verify-media` | Real production software, generated input, independent decode; not capture Agent/SSH |
| Nix packages and gates | Hosted `Native package`, `Delivery scripts` | Build/distribution identity, not runtime cutover |
| Real PoC | [Live acceptance](memory/session-state.md) | Decoded two-host output and heard real chat, with open acceptance gates |

Check the repo-local verifier's current CLI before using an optional target. Opt-in runs require the matching dependencies and containment profile; do not run Python fixture internals directly or mistake selected-case exit 0 for the full acceptance profile.

## Findings attached to immutable runs

| Identity | Confirmed result | Limitation or retained failure |
| --- | --- | --- |
| 2026-11-13 source repair | Historical 102 native tests, private audio, source repair reviews | Not rerun by extraction |
| S14/E07–E10 2026-11-14 standalone | 110 normal tests, native/WASM artifacts, 22 Nix evaluator assertions, 5 targeted consumer gates | Whole consumer flake failed at unrelated Firefox theme derivation, also reproducible at untouched baseline |
| S16/E12 virtual | 14-stage virtual test, stand-in workers and software transport | Not Agent RTMP/RTSP physical chain; revised oracle rejects zero-test opt-in |
| S18/E14 delivery `d2205d0` | Signed GitHub Releases cache release `43561723796` and fresh no-build consumption | Only named input and architecture, not current arbitrary closure or deployment |
| S19/E15 merged PR #2 | Listener-side urgent Stop and owner retry tests | Accepted Stop intent ≠ completed cleanup, parser exhaustion/cgroup crash not closed |
| S20/E16 `42a64e4` | Agent/MediaMTX/RNNoise/OBS production-only generated media; actual MKV decode/negative controls | Original AV01 phase-fit claim corrected; first Prepare reset had unknown cause |
| S22/E17 `fb632bf` | Six media cases, 16 oracle units, six mutation controls, 20 reset probes | Non-recurrence ≠ reset fix; no physical acquisition |
| S24–S25/E19–E20 `3c2f913` | Local finite WAV player fixture and ordinary 11-stage matrix, 165 tests per feature selection | First convergence full run had three two-agent resets; subsequent source changed; not Twitch or physical listening |
| S26/E21 `ed7fe60` | 4 hosted jobs success at main; PR #9 merged | `poc-play` is local fixture only; PoC still incomplete |

The failed [run 44089362405](https://github.com/tenzyu/splitcast/actions/runs/44089362405) remains failed historical evidence; PR #6's eventual async Stop-test fix does not change that run's result. Source revision, test environment, actual command, output artifact and acceptance contract must travel together; do not sum test counts across matrices or mark stale runs as current.

## Still unproven

Matched workload capture non-regression across input, audio, update, draw and
display. Bound a live decoded freshness envelope and independently align the
physical mic and game stream. Exercise media faults with real capture,
confirm clean OBS process exit and repeat the acceptance on a source-bound
packaged build. [Current gates](memory/session-state.md)

**Provenance:** [ID register](memory/evidence.md) · [real media receipt](evidence/media-plane-20261115.md) · [PR #3 oracle review](evidence/pr3-review-20261117.md) · [PoC convergence](evidence/poc-convergence-20261117.md) · [S26](evidence/dreaming-main-20261117.md) · [archived full log](https://github.com/tenzyu/splitcast/blob/archive/pre-dreaming-20261117/docs/verification.md).

## 2026-11-18 runtime performance and operational acceptance (C)

The worktree started at `e92b5db390835041b33de46618c49d5f98cad9cf`.
The owner-only convergence contract `runtime-acceptance-c` has SHA-256
`9a38c679e0a5a523e7c8a8c7eca92cd9992d29b41c1f0a11ece118068cf6d957`.
The completed measurement implementation is
`dc730aa2cd3892b16ce97552e874d060a71a82bf`, convergence snapshot
`a6944958d0dd803719091cd816e3972e11d635bf2dd7db737f2416a40e5acfec`.
The earlier implementation `9a037b2ccb8e9cacf7f413f185e0651f3036ffe3`
used snapshot `9eb67ff280ad9244d84a1ebf1b1f2814a79abf808e4d706691ed0214dfbcbf76`.
The evidence append changes documentation only.
Private `attempt-*.json` and logs are under the worktree's Git metadata.

| Exact command | Result |
| --- | --- |
| `direnv allow` | Exit 0 after reviewing `.envrc`. Checks below used the explicit Nix wrapper. |
| `nix develop --no-write-lock-file .#media-test -c bwrap --unshare-all --die-with-parent --new-session --ro-bind /nix/store /nix/store --dev /dev --proc /proc --tmpfs /tmp -- bash -c 'test ! -e /dev/dri && test ! -e /dev/snd'` | Exit 0 from the preliminary unprivileged namespace probe. |
| `nix develop --no-write-lock-file .#media-test -c python3 -c 'from inspect import signature; from websockets.sync.client import connect; print(signature(connect))'` | Exit 0 on the pre-commit media shell. The pinned client is websockets 16.0. |
| `nix develop -c cargo run -p splitcast-xtask -- verify` | Earlier commit exit 0, 10/10 checks (`attempt-000002`, verifier run `1794983560056512408`). At the completed implementation, first exit 1, 8/10 checks (`attempt-000008`, run `1794984033172964580`). Same-source retry exit 0, 10/10 checks (`attempt-000014`, run `1794984411693602261`). Rust 1.95.0, x86_64 Linux. Exact Cargo commands and exits are in each `artifacts/verification/runs/<run>/summary.tsv`. The ordinary verifier left opt-in integrations ignored. |
| `nix develop --no-write-lock-file .#media-test -c python3 -m unittest discover -s tools -p test_media_acceptance.py -v` | Exit 0, 20 tests on the earlier implementation and 21 on the completed implementation. Receipts `attempt-000003` and `attempt-000009`. |
| `bash -n tools/verify-media.sh` | Exit 0 before commit and in receipts `attempt-000004` and `attempt-000010`. |
| `python3 tools/ci/check_docs.py` | Exit 0 before commit and in receipts `attempt-000005` and `attempt-000011`. 49 Markdown documents, 267 local links, zero broken links and OKF issues at the implementation commit. |
| `git diff --check` and `git diff --cached --check` | Exit 0 during pre-commit review. |
| `git diff --check e92b5db390835041b33de46618c49d5f98cad9cf` | Exit 0. Receipts `attempt-000006` and `attempt-000012`. |
| `nix develop --no-write-lock-file .#media-test -c cargo run -p splitcast-xtask -- verify-media` | Pre-commit exit 0, 20 oracle tests and all seven device-free cases. Receipt `attempt-000001`, media run `20261118T062616Z-mFWYuCDV`. Earlier clean implementation exit 0, 20 tests and seven cases, receipt `attempt-000007`, run `20261118T063501Z-8JI0gF3j`. The identical clean snapshot reused that receipt. Final implementation exit 0, 21 tests and seven cases, receipt `attempt-000013`, run `20261118T064143Z-uQwvIGaN`. Media receipts are under `artifacts/media-plane/runs/`. |

After the evidence append, `python3 tools/ci/check_docs.py` exited 0 with
49 Markdown documents, 271 local links, zero broken links and OKF issues.
`git diff --check e92b5db390835041b33de46618c49d5f98cad9cf` exited 0,
and the worktree scope check found no violations.

At `attempt-000008`, `cargo test --workspace --locked --offline` exited
101. In `completed_stop_is_not_a_cross_host_prepare_replay_fence`, Prepare
returned 409 with `external service readiness timeout`. The test-owned
MediaMTX stand-in logged `AddrInUse` in
`artifacts/verification/two-agents-165649-0/production/mediamtx.log`.
`cargo test --workspace --no-default-features --locked --offline` also
exited 101. The OBS mock test
`obs::tests::compound_configuration_uses_one_operation_budget` timed out
at connect with `OBS control deadline exceeded`. The Rust sources match
the base for both tests. No cause was established for the OBS deadline miss.

### Generated OBS and age observations

Media source manifest `0af9414f5323eb46effea4ab4d7d99b84e7393e51f2926515f23e3d0df55100d`, Agent SHA-256
`2ac7a71f86c99fb26ea0dd319d18ae24e88c4f62fe4c00ebf90cb85a83147eb0` and OBS closure inventory SHA-256
`be96d04d6f1e4925dcbd924e9446efdf8e7d33db34b5db1ecaf7bd39abefe712`. The private namespace entered with only
loopback and no physical media devices. OBS 32.1.2 ran with Xvfb/Mesa
llvmpipe, x264 and software input decode at 1280×720/30 fps. The launcher
matched its host source receipts before and after. Each observed recording
kept ten authenticated snapshots from one OBS instance including a
Running sample, a warm baseline 2.001–2.009 seconds after the Running
observation and 7.995–8.002 seconds of steady sampling before Stop or
injection.

| Case | Late render / attempts | Output skips / attempts | Non-skipped output frames | Scene age max lower–upper, seconds |
| --- | ---: | ---: | ---: | --- |
| normal | 38/240 (15.83%) | 0/241 | 241 | 1.997–2.164, WITHIN_LIMIT |
| camera | 1/240 (0.42%) | 0/240 | 240 | 1.345–1.523, WITHIN_LIMIT |
| frozen | 5/241 (2.07%) | 0/240 | 240 | 18.553–18.652, STALE |
| delay | 0/240 (0.00%) | 0/240 | 240 | 4.992–5.145, STALE |
| disconnect | 17/240 (7.08%) | 0/240 | 240 | 1.142–1.339, WITHIN_LIMIT |
| stall | 0/240 (0.00%) | 0/240 | 240 | 1.177–1.323, WITHIN_LIMIT |
| worker-death | 0/240 (0.00%) | 0/240 | 240 | 1.539–1.689, WITHIN_LIMIT |
| worker-death recovery | 0/240 (0.00%) | 0/240 | 240 | 1.878–2.029, WITHIN_LIMIT |

The frozen game feed's ingress advanced from 197,648 to
332,482 bytes, and the delay feed from 208,519
to 396,403 bytes, increasing at all ten observations.
The independently decoded frozen recording rejected
`frozen content progression gap` after the cadence check. The delayed
recording decoded to EOF with 11.8
seconds of content progression. Both scene-age lower bounds exceeded the
three-second fixture ceiling. The negative cases pass by detecting stale
content while the Agent still reports output progress. Its production
control has no trusted age signal.

The first full run measured 124/240 late renders on the worker-death
recovery recording with 0/241 output skips. The next clean run measured
8/240 late renders in the camera case with 0/240 output skips, and 0/240
late renders on recovery. These are software observations. The earlier
physical 117/645 render failure, SIGSEGV and Xorg service Stop timeouts
remain recorded. [PR #16 OBS evidence](evidence/pr16-obs.md) ·
[Physical and recovery evidence](evidence/pr16-managed-acceptance.md)

The normal and camera recordings independently decoded to EOF and
confirmed inactive OBS output, exit code 0, and release of the owned OBS
PID and loopback listener. Across the cases, the fixture recorded no
cleanup errors or fallback SIGKILL in `cleanup.json`. The worker-death
case intentionally killed the owned RNNoise process to test recovery.
Routine fixture teardown terminated the owned Agent and Xvfb. Namespace
execution returned 0. These receipts cover isolated fixture cleanup.
Service cgroup release on the intended hosts remains a separate gate.


### Gates left open

- NOT RUN on the intended hardware. The fixed osu! replay OFF/ON/OFF
  gameplay p99 delta, continuous Direct Scanout, local game audio, and
  input latency need source-bound instrumentation. The historical
  discrete samples keep their original scope.
- BLOCKED on hardware preflight. OBS renderer, active NVENC, hardware
  decode of each input, and the ≤1% post-warmup late-render gate remain
  unverified on the production host. The failed physical Xorg path was
  excluded from this work.
- NOT RUN for physical game/camera frame age, physical A/V marker alignment,
  and a real on-air EventSub → VOICEVOX → capture-side physical return.
- NOT RUN for owner-specific Stop, output inactivity, normal OBS exit,
  process/listener/cgroup release, and fault recovery on the intended
  hosts with this source.
- NOT RUN for `verify-virtual`, a standalone Nix package build,
  `nix flake check`, Miri, and fuzzing in this work unit. The device-free
  checks above cover the scoped measurement oracle and packaged media
  path. No deployment or system-generation change was authorized or
  performed.

Keep the runtime candidate and autostart disabled. See the bounded
[physical acceptance procedure](operations.md#finite-matched-acceptance)
and [current gates](memory/session-state.md).
