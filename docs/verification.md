# Verification — source-bound evidence, not a global PASS

**Historical public main checkpoint:** `ed7fe60e8efd0dea454346b13f38a680096fb60e`. GitHub [run 44097472963](https://github.com/tenzyu/splitcast/actions/runs/44097472963) completed with **four successful jobs**: `verify`, `verify-virtual`, `Native and WASM packages`, and `Delivery scripts`. No live Twitch/game/microphone/output/playback or deployment is covered by that result. [Original verification transcript](https://github.com/tenzyu/splitcast/blob/archive/pre-dreaming-20261117/docs/verification.md) preserved by source ref.

The [audio return receipt](evidence/audio-return.md) records the two-Agent software result, failure injections and independent private-sink observations on the audio return candidate.

## 2026-11-18 native PoC candidate

The live trial and independent observations are summarized in the
[versioned Wiki](memory/session-state.md). The original PR #15 speech receipt
remains unchanged.

| Exact command or profile | Observed result |
| --- | --- |
| `nix develop -c cargo run -p splitcast-xtask -- verify` | PASS on the native media/EventSub candidate before the gain and mic readiness follow-ups. |
| `nix develop -c cargo run -p splitcast-xtask -- verify-virtual` | First isolated run failed because the fixture still used the removed mutable routes command. The final run passed all 14 checks, including real private OpenSSH/PipeWire. Receipt `artifacts/verification/runs/1794966010817819864/`. |
| `nix develop .#media-test -c bash tools/verify-media.sh --case normal` | Initial normal case failed the mic dropout oracle with five consecutive windows. It passed after Start began waiting for the owned RNNoise stream on the private sink. The diagnostic subset leaves the aggregate result INCOMPLETE by design. Receipts `20261118T012927Z-zIVNZMBr` and `20261118T014007Z-kpinXt33` under `artifacts/media-plane/runs/`. |
| `python3 -m unittest discover -s tools/ci -p 'test_*.py'` | 12 PASS, 1 opt-in test skipped. The first run exposed a fixture variable lost when removing the WASM cache root. |
| `ffmpeg -nostdin -v error -xerror -i <trial-mkv> -map 0:v:0 -map 0:a:0 -f null -` | Exit 0 on the real two-host recording. Independent FFprobe frame/packet timestamps, decoded frame hashes and sample counts are retained under `artifacts/mission/`. The recording and derived preview were removed after inspection. |

The source comparison counts physical lines in shipped Rust, native schema
and browser assets. It excludes test-only modules, fixtures, archives and
generated artifacts. Shipped code went from **7,375 to 6,408 lines** (−967,
13.1%) and from **26 to 21 source files**. The workspace went from four
packages to two. The Linux native crate tree grew from 57 to 70 resolved
package/version entries as the existing WebSocket and HTTP clients gained
certificate-verified TLS for Twitch. The reduction removes the TUI, browser
assets and crate, shared UI protocol crate, camera, public OBS publishing,
mutable route/settings persistence and legacy speech file/ack queue.
EventSub owns its WSS, identity validation, migration, bounded admission
and replay set. Authenticated control, session fencing, Stop reconciliation,
worker ownership, expiry and isolated playback remain product requirements.

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
