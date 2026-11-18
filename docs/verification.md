# Verification — source-bound evidence, not a global PASS

**Historical public main checkpoint:** `ed7fe60e8efd0dea454346b13f38a680096fb60e`. GitHub [run 44097472963](https://github.com/tenzyu/splitcast/actions/runs/44097472963) completed with **four successful jobs**: `verify`, `verify-virtual`, `Native and WASM packages`, and `Delivery scripts`. No live Twitch/game/microphone/output/playback or deployment is covered by that result. [Original verification transcript](https://github.com/tenzyu/splitcast/blob/archive/pre-dreaming-20261117/docs/verification.md) preserved by source ref.

The [audio return receipt](evidence/audio-return.md) records the two-Agent software result, failure injections and independent private-sink observations on the audio return candidate.

## 2026-11-18 Studio control contract (A)

The [A-to-B contract](studio-control-contract.md) was pinned at
`df0768f3f8076ced0e3214170ea3113d4459c40d` over
`e92b5db390835041b33de46618c49d5f98cad9cf`. The implementation uses real
local Agent HTTP endpoints, synthetic EventSub notifications, and
authenticated OBS WebSocket stand-ins. The shared screenshot fixture fix is
`bb3d5ea103c7510986aecce6278f2d5e51888e93` and makes both PNGs decodable.
The wire contract is unchanged from the checkpoint.

| Exact command | Observed result |
| --- | --- |
| `direnv allow` | Exit 0 after reviewing the existing bounded devShell entrypoint. Checks below used explicit Nix wrappers. |
| `nix develop -c python3 tools/ci/convergence.py init "$contract" --accept "$reviewed_sha256"` | First enrollment blocked with exit 2 because the manual gate keys differed from the three required acceptance axes. Corrected the unregistered JSON and enrolled the owner-only work unit `studio-control-contract-a` with digest `ba0dba0fd88288b8654410c958a0c6cd247daabce71bcda6b915319334f8c146`. |
| `nix develop -c cargo fmt --all` | Initial formatting attempts failed on a misplaced EventSub module comment (E0753) and a malformed scene reload edit. Both were corrected. Subsequent formatting and verifier fmt checks exited 0. |
| `nix develop -c cargo check --workspace --all-targets --locked --offline` | Exit 0 on intermediate source, with an unused synthetic source warning before the regression tests were connected. |
| `nix develop -c cargo test -p splitcast-rs-core --locked --offline studio_ -- --nocapture` | First completed run passed four tests. The next passed five, including the capture-to-production forwarding and withheld OBS acknowledgment Stop case. Earlier compound fmt/test commands short-circuited on fmt failures, so tests did not run in those attempts. |
| `nix develop -c ffmpeg -nostdin -v error -xerror -i "$fixture_dir/studio-fixture-1.png" -i "$fixture_dir/studio-fixture-481.png" -map 0:v -map 1:v -f null -` | Exit 0 decoding both exact generated PNG byte sequences from the owner-only temporary fixture directory. The 1×1 and 481×1 RGBA scanlines inflate to 5 and 1,925 bytes, respectively. |
| `nix develop -c cargo run -p splitcast-rs-core -- validate --config packages/splitcast-rs/example.toml` | Exit 0, `valid:true` for capture. |
| `nix develop -c cargo run -p splitcast-rs-core -- validate --config packages/splitcast-rs/example-production.toml` | Exit 0, `valid:true` for production. |
| `python3 tools/ci/check_docs.py` | Exit 0 on the implementation checkout, with 50 Markdown files, 260 local links, zero broken links and zero OKF issues. |
| `git diff --exit-code e92b5db390835041b33de46618c49d5f98cad9cf -- flake.nix nix/modules` | Exit 0. The migration gate and manual service definitions match the pinned base. This is a source comparison. |
| `nix develop -c cargo run -p splitcast-xtask -- verify` | Initial exit 1 at `d3829cb6dcb375f34227f7a12cb381f63cc5a14b`. Clippy exited 101 on `clippy::question_mark`. The other nine checks passed. Worktree receipt `attempt-000001`, artifact `artifacts/verification/runs/1794983956294301680/`. |
| `nix develop -c cargo run -p splitcast-xtask -- verify` | Exit 0, all 10 checks passed at `231ad1154a7628b6f8c8e2cab8f483e2c1cec405` with the decodable screenshot fixture. Workspace tests and no-default-features each included the five A regressions and the existing CLI, TUI and Web control fixtures. Worktree receipt `attempt-000002`, artifact `artifacts/verification/runs/1794984271591672491/`. |

The targeted cases cover chat with speech off, independent fan-out, unchanged
speech text and expiry, duplicate handling, UTF-8 limits, history gaps and
Stop under concurrent reads. Scene cases cover bearer and owner rejection,
generation races, the observed canvas and source geometry, legacy and small
preview, OBS reply loss and partial effects, readback, save failure, reload
and Agent restart. The forwarding case measured Stop admission within the
500 ms fixture assertion and separately observed completion and cleanup.

Live Twitch, physical devices and A/V, production OBS rendering and frame
age, matched gameplay and hardware render performance, real two-host
recovery, package builds, post-rename directory sync fault injection, and
deployment were not run for A. The optional Chromium integration and
`verify-virtual` and `verify-media` were not run. The test worker simulates
OBS state and screenshot responses. The physical gates in the
[session state](memory/session-state.md) remain open.

### Two-Agent Prepare timeout follow-up

[Hosted `verify` job 134632547127](https://github.com/tenzyu/splitcast/actions/runs/44254418762/job/134632547127)
failed `sigterm_running_capture_releases_both_agents_workers` during Prepare.
The GitHub merge checkout `badb5010b52d88b0a94b7ecce8fd4256b768b760`
had tree `7038774c9099c42f187d2cd274c4c66ce5b05162`, identical to PR head
`8557945d9c8a35732014f2a766c2e45e2fc348cc`. The retained `tests.log` shows
HTTP 409 with an unknown outcome after transmission began and an operation
I/O deadline error. The MediaMTX stand-in log was outside the upload allowlist.

| Exact command | Observed result on `8557945d9c8a35732014f2a766c2e45e2fc348cc` |
| --- | --- |
| `nix develop -c cargo test -p splitcast-rs-core --locked --offline --test two_agents sigterm_running_capture_releases_both_agents_workers -- --exact --nocapture` | Exit 0, one passed on the clean checkout. |
| `nix develop -c cargo test -p splitcast-rs-core --locked --offline --test two_agents -- --nocapture` | A clean standalone suite passed 16 tests with four ignored. Four concurrent invocations of the same suite produced two passes and two failures. Two fixtures in one test process chose MediaMTX port 40713. One stand-in failed its bind with `EADDRINUSE`; its Agent accepted the other fixture's readiness response. A separate collision failed an OBS Start. |
| `nix develop -c cargo test -p splitcast-rs-core --locked --offline --test two_agents -- sigterm_running_capture_releases_both_agents_workers reproduce_prepare_timeout_with_fixture_port_taken --nocapture` | The saved test-only patch retained the old allocator. Its new exclusivity assertion failed because the receiver port was free to bind. A second test took that port with a loopback listener and reproduced the hosted 409 text, confirmed `EADDRINUSE` in the worker, and verified explicit Stop, production Idle, and released workers. The overall invocation exited 101. |

The fixture now keeps each loopback port assigned through both Agents'
teardown. A bound socket without a listener holds the port out of automatic
allocation and `SO_REUSEADDR` permits the assigned worker to bind it. The
SIGTERM regression checks the reservation before Prepare and still checks
that workers and listeners are gone after shutdown. The captured failure
establishes this local cause for the same 409; the initial hosted job lacks
worker evidence to establish its exact cause. Local raw logs, the temporary
patch, and the original hosted upload are under `artifacts/pr-19-prepare/`.

## 2026-11-18 SplitCast Studio Native (B)

Stack base is `feat/studio-control-contract` at
`8557945d9c8a35732014f2a766c2e45e2fc348cc`, the published head of
[draft PR #19](https://github.com/tenzyu/splitcast/pull/19) into `main`.
B was enrolled from contract checkpoint `df0768f3f8076ced0e3214170ea3113d4459c40d`,
integrated locally, then replayed on the published implementation including
the valid generated PNG fixture. The checked B code and fixture are
`0d9f6f0fd935aa9ac1f5b41a246650c4f3298a13`, tree
`f944c9d94e99b365aa787b1419c947b2c7f519e6`. This evidence entry was added
after the checks. The comparison against #19 contains the optional Studio
package, its flake entry points, and this record.

The owner-only worktree contract `splitcast-studio-published-a-b` has
digest `c94dadfc436c5e18013c3de76ecb5d17be97991c47d9ebfb567a3e00f3db59ad`.
The clean source snapshot was
`7124ed3aa27b9bc344d357507640f8b39b9b0377116da51a1edc5a432eaa1dcf`.
The convergence scope had no violations. A repeated GUI check in the same
shell reused `attempt-000011` without launching the fixture again.
Physical functional, gameplay and operational gates are still unverified.

| Exact command | Observed result |
| --- | --- |
| `nix develop -c cargo run -p splitcast-xtask -- verify` | Exit 0, all 10 checks passed. Convergence `attempt-000004`, xtask `artifacts/verification/runs/1794986387669825093`. The earlier run at `1aca148` also passed all 10, in run `1794984782703905050`. |
| `nix develop .#studio -c cargo fmt --manifest-path packages/splitcast-studio/Cargo.toml -- --check` | Exit 0, `attempt-000003`. |
| `nix develop .#studio -c cargo clippy --manifest-path packages/splitcast-studio/Cargo.toml --all-targets --locked -- -D warnings` | Exit 0, `attempt-000005`. |
| `nix develop .#studio -c cargo test --manifest-path packages/splitcast-studio/Cargo.toml --locked` | Exit 0, 5 passed. Covers current Start ID and Live confirmation, Stop proof, hidden/stale reads, chat bounds, and oversized PNG rejection. `attempt-000006`. |
| `nix flake check --no-build` and `nix flake check` | Both exited 0. Evaluation emitted the existing `homeManagerModules` output warning. The full check built the native package and validated configurations. Evaluation receipt `attempt-000007`. |
| `nix develop -c python3 tools/ci/check_docs.py` | Exit 0, `attempt-000008`. |
| `nix develop -c cargo audit --file packages/splitcast-studio/Cargo.lock` | Exit 0, no vulnerability findings in the database used. Seven unmaintained dependency warnings: `bincode`, `chrono`, `pin-utils`, `ttf-parser`, `zbus_names`, `zvariant`, and `zvariant_derive`. `attempt-000009`. The earlier GPUI lock audit exited 0 with 14 unmaintained warnings. |
| `nix build .#splitcast-studio --no-link` | Exit 0 for both candidates on their respective source. Slint receipt `attempt-000010`. |
| `nix run .#splitcast-studio -- --help` | Exit 0, printed `--token-file`, `--port`, and optional `--agent-port` usage. |
| `nix eval --raw "git+file://$PWD?rev=$base#packages.x86_64-linux.default.drvPath"` and `nix eval --raw .#packages.x86_64-linux.default.drvPath` | With `base` set to A's full published SHA, the Agent's default derivation matched exactly (`vs4s8i1vy2s79pmn2m5h4mx2fn4sr73d`). |
| `nix develop -c cargo run -p splitcast-xtask -- portability` | Exit 0. |
| `nix develop .#studio-test -c python3 packages/splitcast-studio/tests/gui.py` | Exit 0 on Slint, `attempt-000011`. Confirmed HTTP 202 Stop admission followed by matching completed Idle and production release. Sampled four preview requests, with minimum gap 1.200 s. |
| `nix develop .#studio-test -c python3 packages/splitcast-studio/tests/gui.py "$(nix path-info .#splitcast-studio)/bin/splitcast-studio"` | Exit 0 using the Nix package binary. Repeated the GUI workflow, HTTP 202 and matching completion, and the 1.200 s minimum preview gap. |

### Release GUI measurements

The harness ran on Linux 6.18.63 x86_64 with Rust 1.95, one private
authenticated Xvfb 21.1.24 X11 screen at 1440 × 920 × 24 per run, DejaVu
fonts, and approximately 1110 × 780 Studio windows. It focused the window
with `xdotool`, connected through an authenticated loopback relay to a real
Idle production Agent with a camera draft, and applied a local overlay
draft before sampling. Each run settled for three seconds and sampled
`/proc/PID/smaps_rollup` once per second for 20 samples, with process
user plus system ticks from `/proc/PID/stat` divided by elapsed wall
time and `SC_CLK_TCK`. CPU is a percentage of one logical core. Renderer
traces count GPUI root renders or Slint Winit `RedrawRequested` events.
The idle sample covers a focused window after Apply, with keyboard focus
out of text editing. The normal platform caret while editing is outside
that sample. Local build activity was present during the first GPUI and
Slint runs.

| Artifact and backend | Elapsed | Idle PSS range | Idle CPU | Idle redraws |
| --- | ---: | ---: | ---: | ---: |
| Cargo release GPUI 0.2.2, X11 Blade/Vulkan on Mesa 26.1.8 llvmpipe (LLVM 21.1.8, `LP_NUM_THREADS=2`), `1aca148` | 20.073 s | 103.858–134.346 MiB | 0.448% | 0 |
| First Cargo release Slint 1.19.0, X11 Winit software, `4189edf` | 20.025 s | 20.447–20.494 MiB | 0.100% | 0 |
| Checked Cargo release Slint 1.19.0, X11 Winit software, `0d9f6f0` | 20.016 s | 20.728–20.761 MiB | 0.100% | 0 |
| Nix package Slint 1.19.0, X11 Winit software, `0d9f6f0` | 20.020 s | 20.888–20.888 MiB | 0.100% | 0 |

GPUI exceeded the 80 MiB PSS limit in every sample. Slint fits all three
idle targets in this display fixture and is the sole Studio implementation
in the final tree. The published GPUI crate enables Blade for the X11 and
Wayland backends. Slint is explicitly built and selected for Winit software
rendering. [GPUI 0.2.2 features](https://docs.rs/crate/gpui/0.2.2/features) ·
[GPUI upstream Linux notes](https://github.com/zed-industries/zed/tree/main/crates/gpui) ·
[Slint 1.19.0 Cargo features](https://github.com/slint-ui/slint/blob/v1.19.0/api/rs/slint/Cargo.toml) ·
[Slint attribution terms](https://github.com/slint-ui/slint/blob/v1.19.0/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md)

### GUI evidence and limits

- A direct production connection preserved a draft edited before connection,
  applied it in Idle, and applied a camera-guide drag in canvas pixels.
  The two-Agent portion used matching camera-free configurations and stayed
  on private loopback through local stand-ins. The relay preserved the
  Agent's original Host and Origin port while using a different local
  connection port.
- The client selected Live without dispatch and then started Recording with
  the current Start ID. The owned OBS stand-in produced a valid generated
  PNG. Requests stopped with the editor hidden, the preview hidden, and
  after Stop. The visible chat cursor used the authenticated route and
  consumed a single injected relay delta. Hidden chat made zero requests.
  Empty deltas and static snapshots produced zero redraws over a further
  4.2 s window during Recording.
- An external generation change produced a conflict and preserved the local
  edit. An explicit Apply, OBS reconciliation, Save and Reload verified the
  draft/readback/saved states. A dropped response produced unknown outcome
  before a fresh observation. A delayed Apply response arrived after Stop
  and left the earlier operation unconfirmed. Status and a simultaneous
  second Stop reached the Agent while Apply was pending. The fixture
  observed matching completed Idle, released remote ownership, inactive
  owned output, exited stand-ins, and disconnect presentation.

The first checkpoint enrollment rejected noncanonical manual-axis names
(exit 2). Enrollment with the repository's three axis names succeeded.
The local integration GUI attempt initially stopped at Python import due to
an HTTP helper shadowing `http.server`
(`splitcast-studio-integration-local-a-b/attempt-000001`).
On the first published-base GUI run, GPUI completed the idle sample and the
fixture later timed out waiting for Ready with camera configured on
production only (`splitcast-studio-published-a-b/attempt-000001`).
The passing Slint run separates the Idle camera draft experiment from the
matching two-Agent Start configuration (`attempt-000002` at `4189edf`,
then the tightened `attempt-000011`).

Initial `nix develop .#studio -c cargo build --manifest-path packages/splitcast-studio/Cargo.toml --release --locked -j 6`
failed because GPUI's `xattr 0.2.3` used the Linux `ENOATTR` alias removed
from libc 0.2.191. The first
`nix develop .#studio -c cargo update --manifest-path packages/splitcast-studio/Cargo.toml -p libc --precise 0.2.189`
also failed because mio 1.2.5 required libc 0.2.190 or later. Pinning mio
1.2.4 allowed the update. GPUI build retries with `--release --locked -j 8`
then found GPUI API and a borrowed-label lifetime error before the release
artifact compiled. Slint's initial
`nix develop .#studio -c cargo build --manifest-path packages/splitcast-studio/Cargo.toml --release -j 8`
failed on missing `fontconfig.pc`. The next locked run found the
`LayoutAlignment.start` name collision. The final build passed after
adding the build input and qualifying the enum.

The GUI integration covers software rendering with stand-ins and an injected
chat delta. A real upstream Twitch chat event, real OBS media and recording,
Wayland window operation, physical devices, real SSH, and host rollout or
recovery remain unrun. Miri, fuzzing and an independent Studio Cargo Deny
configuration were not run. Existing Nix runtime gating and service
activation require separate operator authorization. For rollback, close
Studio and use the existing CLI, TUI or Web with explicit Stop and
completion observation when needed. Revert B as a unit to remove Studio's
optional entry points. Saved layouts and the running session are owned by A.

### Native Wayland focus follow-up

The worktree started clean at `04f60e32a8aaf6103a568000c231da53d35ffe5c`.
Local merge `6a71b3bf618c0c6668e487c602653fdb7a193954` incorporates #19
`51bd829d56169b930d60315d2682f0c4c6eec2f9` and preserves both earlier
verification sections. The checks and GUI harness ran on clean checkout
`fb847b8bc299aed18a11f0c122eec409a9546d0d`, tree
`e34b61358bacd5b651e538085427fec588239e57`. The candidate Studio package
was built from `aab9907d53ddb4be71c63ab66251d938a64580a5`.
`nix path-info .#splitcast-studio` on the tested checkout selected the
same package. The release ELF SHA-256 values are
`09219610af6f2805952dabf657dc8a2350874b51f6073f31fe9059a5ebdde687`
before and
`9be436ec41a05aaed4a9e839a57375ae2704a81db4cbae31c32964a597656cc3`
after. This verification entry follows the measured source.

| Exact command | Observed result |
| --- | --- |
| `nix develop -c cargo run -p splitcast-xtask -- verify` | Exit 0, all ten checks passed. Convergence `attempt-000014`, xtask `artifacts/verification/runs/1794992463672618093`. |
| `nix develop .#studio -c cargo fmt --manifest-path packages/splitcast-studio/Cargo.toml -- --check` | Exit 0, `attempt-000009`. |
| `nix develop .#studio -c cargo clippy --manifest-path packages/splitcast-studio/Cargo.toml --all-targets --locked -- -D warnings` | Exit 0, `attempt-000010`. |
| `nix develop .#studio -c cargo test --manifest-path packages/splitcast-studio/Cargo.toml --locked` | Exit 0, six passed, including focus loss preserving read revisions and cadence and occlusion invalidating a pending read. `attempt-000011`. |
| `nix flake check --no-build` | Exit 0, `attempt-000012`. |
| `nix develop -c python3 tools/ci/check_docs.py` | Exit 0, `attempt-000013`. |
| `nix eval --raw "git+file://$PWD?rev=$base#packages.x86_64-linux.default.drvPath"` and `nix eval --raw .#packages.x86_64-linux.default.drvPath` | Exit 0 with `base` set to confirmed #19. The default Agent derivations matched exactly at `zwsk90fgmac05rmpzfprdrjcbzipizj2`. |

The convergence contract is `studio-wayland-release`, digest
`f2a5acb757dd8268e82fbad2eea0f36dff0f1e17a7b8a4175fba49e3521d7040`.
The exact GUI runs used the following commands and logged stdout and
stderr in `artifacts/studio-wayland/{before,after}.log`.

```sh
studio_before=04f60e32a8aaf6103a568000c231da53d35ffe5c
studio_after=aab9907d53ddb4be71c63ab66251d938a64580a5
before_package="$(nix build "git+file://$PWD?rev=$studio_before#splitcast-studio" --no-link --print-out-paths)"
after_package="$(nix build "git+file://$PWD?rev=$studio_after#splitcast-studio" --no-link --print-out-paths)"
mkdir -p artifacts/studio-wayland
nix develop .#studio-wayland-test -c python3 packages/splitcast-studio/tests/wayland.py --studio-binary "$before_package/bin/splitcast-studio" --studio-source "$studio_before" --focus-only --evidence artifacts/studio-wayland/before
nix develop .#studio-wayland-test -c python3 packages/splitcast-studio/tests/wayland.py --studio-binary "$after_package/bin/splitcast-studio" --studio-source "$studio_after" --evidence artifacts/studio-wayland/after
```

Both Nix builds exited 0. The GUI used unprivileged headless Sway 1.12 at
1740 × 920 with Pixman and Xwayland disabled, Slint 1.19.0 / Winit 0.30.13
software rendering, and a separate foot 1.27.0 toplevel. The protocol
record shows Studio creating an `xdg_toplevel` and `wl_shm` buffers,
then Studio receiving `wl_keyboard.leave` and foot receiving
`wl_keyboard.enter`. Sway reports both as visible native windows at
disjoint rectangles before and after injection. The test forwarded Studio's
cursor and bearer-authenticated request to the real local Agents before
substituting the synthetic chat response. Preview loaded a generated
480 × 270 PNG after an authorized stand-in OBS response.

- **Before, exit 1.** After handoff, the injected row 129 produced zero
  chat requests and zero preview requests during the six-second observation.
  The chat crop stayed pixel-identical, with row 128 still first.
- **After, exit 0.** During the same handoff, one chat request and two
  preview requests preceded the after capture. Studio rendered row 129
  first, with 128 rows retained and focus still on foot. Across 53 request
  intervals the shortest preview gap was 1.1999 s. The hidden preview and
  editor, hidden chat, and post-Stop intervals each had zero relevant read
  requests.
- The candidate completed the full Wayland workflow. Output selection alone
  sent no Start. Recording required the explicit Start. Camera drag, scene
  conflict, Apply, OBS readback, Save, Reload, a lost reply and a late reply
  were observed through two real Agents with stand-in workers. Stop returned
  HTTP 202 during the held Apply. Studio displayed admission and later
  observed completed Idle with the same stop ID and
  `remote_release_pending=false`. Production was Idle and its owned
  output and workers were gone.

Each resource sample followed three seconds of warmup and covers 20
one-second PSS readings from `/proc/PID/smaps_rollup`. CPU uses process
user and system ticks divided by elapsed wall time and clock ticks per
second. It is the percentage of one logical core attributable to Studio.

| Studio workload | Elapsed | PSS range | CPU | Redraws |
| --- | ---: | ---: | ---: | ---: |
| Before, Idle | 20.019 s | 22.119–22.160 MiB | 0.100% | 0 |
| After, Idle | 20.024 s | 22.151–22.198 MiB | 0.050% | 0 |
| Before, focused with 128 chat rows and preview | 20.026 s | 24.506–24.581 MiB | 0.250% | 0 |
| After, focused with 128 chat rows and preview | 20.023 s | 24.510–24.589 MiB | 0.250% | 0 |
| After, unfocused with 128 chat rows and preview | 20.023 s | 21.499–21.553 MiB | 0.200% | 0 |

The candidate met the 80 MiB PSS and 0.5% one-core Idle targets. Loaded
samples used static content while reads continued. The PSS of the
unfocused run includes library sharing with the second native process.
These samples represent the isolated headless workload.

Raw evidence lives under `artifacts/studio-wayland/`. Each final run
contains its source manifest, Wayland and renderer traces, compositor
trees, full screenshots, chat crops, request intervals, and resource
samples. Earlier nonpassing runs were retained. `baseline-attempt-01`
lacked the virtual pointer, `baseline-attempt-02` and `-03` typed a
literal `a` with the virtual keyboard, and `baseline` delivered the
drag destination after button release. `baseline-native` and
`baseline-focus` observed an earlier Apply label. `baseline-focus-final`
and `candidate-focus` stopped at 128-row setup because the initial
synthetic epoch was invalid upstream. The final fixture forwards a
protocol-valid 32-hex epoch. An unfiltered offline Cargo metadata probe
failed on an uncached Android dependency. The initial parallel
convergence launch encountered the runner lock. Clippy attempt
`000002` rejected a nested match, which was fixed. The earlier verifier
and Studio unit test commands exited 0 but their convergence snapshots
changed during the runs. Attempts `000011` and `000014` establish
the later stable results.

Pinned Winit provides no compositor-driven occlusion observation on
Wayland. The client gates on Slint window visibility, explicit panel
visibility, and occlusion events on backends that report them. Current
X11, physical gameplay and capture, real OBS/Twitch, and deployment
remain outside this run. For code rollback, close Studio or revert the
focus follow-up in a reviewed change. Session control and cleanup remain
with the Agent.

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
