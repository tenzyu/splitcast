# Verification — source-bound evidence, not a global PASS

**Current public main:** `ed7fe60e8efd0dea454346b13f38a680096fb60e`. GitHub [run 44097472963](https://github.com/tenzyu/splitcast/actions/runs/44097472963) completed with **four successful jobs**: `verify`, `verify-virtual`, `Native and WASM packages`, and `Delivery scripts`. No live Twitch/game/microphone/output/playback or deployment is covered by that result. [Original verification transcript](https://github.com/tenzyu/splitcast/blob/archive/pre-dreaming-20261117/docs/verification.md) preserved by source ref.

## Verification entry points

With direnv/Nix shell already entered, execute commands directly; otherwise use the `nix develop -c` wrapper.

| Contract | Command | Scope |
| --- | --- | --- |
| Source/unit/Clippy/WASM matrix | `cargo run -p splitcast-xtask -- verify` | No physical device/stream required |
| Virtual control and private opt-ins | `cargo run -p splitcast-xtask -- verify-virtual` | Actual local controllers and stand-in workers; not physical media |
| Generated production-only media | `cargo run -p splitcast-xtask -- verify-media` | Real production software, generated input, independent decode; not capture Agent/SSH |
| Nix packages and gates | Hosted `Native and WASM packages`, `Delivery scripts` | Build/distribution identity, not runtime cutover |
| Real PoC | Bounded authorized trial defined in [handoff](handoff/poc-20261117.md) | Not yet accepted |

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

Actual GSR/helper and appropriate capture-host microphone source; real capture-Agent → authenticated transport → production decoded media; fresh live-frame policy and cross-host clock correspondence; Twitch `channel.chat.message` v1 → VOICEVOX → capture-host playback; stable all-frame gameplay OFF/ON/OFF and Direct Scanout; real browser action, forced-death cleanup and listener ingress/IPv4/IPv6 containment. Deployment requires its own approval.

**Provenance:** [ID register](memory/evidence.md) · [real media receipt](evidence/media-plane-20261115.md) · [PR #3 oracle review](evidence/pr3-review-20261117.md) · [PoC convergence](evidence/poc-convergence-20261117.md) · [S26](evidence/dreaming-main-20261117.md) · [archived full log](https://github.com/tenzyu/splitcast/blob/archive/pre-dreaming-20261117/docs/verification.md).
