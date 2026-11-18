# Verification — source-bound evidence, not a global PASS

**Historical public main checkpoint:** `ed7fe60e8efd0dea454346b13f38a680096fb60e`. GitHub [run 44097472963](https://github.com/tenzyu/splitcast/actions/runs/44097472963) completed with **four successful jobs**: `verify`, `verify-virtual`, `Native and WASM packages`, and `Delivery scripts`. No live Twitch/game/microphone/output/playback or deployment is covered by that result. [Original verification transcript](https://github.com/tenzyu/splitcast/blob/archive/pre-dreaming-20261117/docs/verification.md) preserved by source ref.

The [audio return receipt](evidence/audio-return.md) records the two-Agent software result, failure injections and independent private-sink observations on the audio return candidate.

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
| Source-bound physical private recording and public Twitch tests | PASS for integrated composited media, decoded viewer playback, private/public Stop and actual OBS exit at `d9b0948`. Physical A/V alignment, p99 gameplay timing, production late-render target and complete Xorg rollback remain open. Post-Stop public-window physical speech playback was not observed. |
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
