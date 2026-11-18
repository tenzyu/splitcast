# Real local media acceptance

This profile tests the production controller with generated inputs. It does not use capture devices, the capture Agent, GSR, SSH, a shared audio server, or an existing OBS session. It does not establish hardware performance or authorize deployment.

## Run the profile

Run from the repository root on Linux with unprivileged user namespaces available.

```sh
nix develop --no-write-lock-file .#media-test -c cargo run -p splitcast-xtask -- verify-media
```

The launcher runs source/build provenance checks, pure unit tests, the default-feature locked/offline native build and OBS closure preflight **on the development host**. These steps are not sandboxed. Only the subsequent actual-media Python profile and its services run inside Bubblewrap's private user, process, mount, and network namespaces. In that profile physical media devices are absent, source is read-only, and only the newly allocated `/evidence` directory is writable through the repository mount. Ephemeral `/tmp` and namespace runtime files are separate. The network contains only loopback. Xvfb disables TCP and requires a private cookie. The existing patched OBS binds its WebSocket to loopback.

The profile writes private evidence under `artifacts/media-plane/runs/`. Keep recordings, credentials, full commands, tool paths, and raw logs private. A missing dependency or unavailable namespace is a failed or blocked gate, never a skipped pass.

For a diagnostic subset, run `bash tools/verify-media.sh --case normal` in the same shell. A subset always returns nonzero because it is not complete acceptance.

## Tested route

```text
generated grayscale frames and game tones
  -> real FFmpeg H.264/AAC publisher -> Agent-owned MediaMTX RTMP /game
  -> MediaMTX RTSP /game -> real OBS GameFeed -> local OBS MKV

generated voiced harmonic audio
  -> real FFmpeg Opus publisher -> MediaMTX RTSP /mic
  -> Agent-owned FFmpeg RNNoise -> private Pulse null sink
  -> real OBS MicProcessed -> the same local OBS MKV
```

Authenticated Prepare and Start drive the real production Agent. The source publishers replace only the physical capture boundary. No OBS-like responder or detached FFmpeg recording can satisfy this profile.

`obs.video_mode = "software"` selects x264 recording and CPU feed decoding. Omission preserves the existing hardware behavior. There is no automatic fallback. The synthetic microphone uses an explicit zero VAD threshold. This tests routing and identifiable processed audio, not speech quality or noise-reduction effectiveness.

## Observable assertions

The declared fixture is 1280 by 720 at 30 frames per second with 48 kHz audio and a ten-second recording window. Publishers have a finite 45-second limit to cover service startup and fault observation. The complete matrix includes normal output, frozen video, a four-second generated-frame backlog before RTMP ingress, disconnected publication, stalled publication, killed RNNoise, and explicit recovery on the same controller after worker failure. The backlog is bounded to 128 MiB. It tests old source content traversing the real route, not delayed network packets or congestion control.

Each generated frame carries a source epoch, a tenth-second ordinal,
calibration bits, and parity. The feeder records the first generation start
and last generation completion for each ordinal. Once frozen, the publisher
repeats identical pixels without refreshing the content timestamp. The
observer asks OBS for a scene screenshot and decodes its marker independently
with FFmpeg. The lower age bound is request time minus last generation
completion for that ordinal, floored at zero. The upper bound is receipt
time minus first generation start. Source generation and screenshot times
use the fixture process's monotonic clock. The bounds include sampling and
RPC uncertainty and cover age at the observed OBS scene. Physical capture
age, scanout, and inter-host latency require their own measurement.

Ignore the first two scene observations for the age verdict. A healthy
fixture has every remaining upper bound at or below three seconds. A negative
case proves staleness when a remaining lower bound exceeds three seconds.
Overlap with the ceiling is inconclusive. The frozen case must independently
reject decoded content progression despite output cadence. The delay case
must decode moving content while its scene age is stale. Both must show
increasing MediaMTX game ingress bytes and advancing OBS video output.
The receipts retain the sampled marker/time bounds and separate progress
observations. Agent production control lacks a trusted age signal and
cannot autonomously enforce this observer's gate.

The generated OBS observer uses one authenticated loopback WebSocket per
recording, including a fresh one for recovery. It reads `GetRecordStatus`
and `GetStats` at Running, then reads the baseline at or after two seconds
from the Running observation and samples through the ten-second window
before Stop or fault injection. It retains timestamps, active and advancing
recording duration, and raw counter values in `obs-stats-*.json`. Keep all
four cumulative counters in the same OBS instance and recording. The
baseline-to-last differences exclude warmup. Invalid, absent, decreasing
or inconsistent counters, an inactive/paused recording, connection loss,
or zero attempted frames make the corresponding measurement unavailable.
A missing baseline or post-warmup sample also leaves the measurement
unavailable. There is no reconnect or cross-session subtraction.

For rendering report `ΔrenderSkippedFrames / ΔrenderTotalFrames` and both
counts. Report output-thread skips and attempts separately, plus
`ΔoutputTotalFrames - ΔoutputSkippedFrames` and the number of intervals
without such progress. Output-thread progress is an OBS-side observation.
An independent decode supplies the output integrity and content evidence.
This fixture uses one OBS output at 30 fps and reports software render
observations without a render-percentage pass threshold. The physical
post-warmup ≤1% gate applies separately to the authorized hardware and
matched workload. [Pinned OBS WebSocket implementation](https://github.com/obsproject/obs-websocket/blob/1fcb95b15aa88b1b7e9bda3f9c8650e314377169/src/utils/Obs_ObjectHelper.cpp) · [OBS 32.1.2 output counters](https://github.com/obsproject/obs-studio/blob/32.1.2/libobs/media-io/video-io.c)

The independent sink check probes streams and packets, checks decode timestamps, fully decodes the MKV to EOF, reads frame identities, and examines decoded PCM. Each source second has a unique game frequency, starting at 440 Hz with a 40 Hz increment. A separate harmonic microphone identity must survive the real processing path. The oracle matches uniquely identified audio transitions against video source ordinals on the decoded presentation timeline. It does not establish microphone alignment or a competitive-streaming latency guarantee.

Audio frame PTS must agree with cumulative decoded samples within 5 ms, allowing container timestamp quantization without silently concatenating missing time. Video and audio analyses retain their distinct first decoded PTS. Their starts, ends, and analysis coverage must agree within 500 ms. A/V transition bounds include a 200 ms baseline uncertainty for the 100 ms analysis windows, with extra uncertainty for missing video observations. Every matched edge's absolute offset plus uncertainty must remain within 500 ms. At least three edges must be observable in both streams. Partial boundary edges outside that shared observation interval are reported separately, not assigned a timing estimate. Every audio analysis window, including both boundaries, must identify source content observed in nearby video. These are sampled content and transition checks, not a continuous per-sample synchronization guarantee.

A normal run must also reject six modified copies of its actual OBS recording: an 800 ms audio stream offset, an internal 800 ms audio PTS gap, incorrect source-second audio in the opening 400 ms and closing 400 ms, 25% dropped-frame VFR, and a 3.5-second notch of the microphone's 180 Hz identity. The last control preserves game audio and must fail the consecutive-missing-microphone check, not just the overall ratio. These are oracle controls, not substitutes for the real positive path. [PR #3 follow-up evidence](evidence/pr3-review-20261117.md) distinguishes fixture failures from successful rejections.

Nominal frame rate and total count are not cadence evidence. Decoded video PTS must be strictly increasing. The steady interval excludes one second of startup and 200 ms at EOF, and requires at least seven seconds, density of at least 28.5 fps, at least 27 frames in every full second, and no inter-frame gap over 101 ms. These allow occasional drops/quantization, not 25% sustained loss. Fault cases apply this check only to the known pre-fault ten-second prefix, never the intentionally broken tail. A frozen source still has regular output cadence and must fail the independent content gate.

Every complete 4,800-sample (100 ms) PCM window is classified, including exact-length final windows. Any partial tail is reported with its sample count and is not classified; it is shorter than 100 ms and actual stream endpoints remain checked. Both identities retain their overall presence thresholds and may be absent in no more than three consecutive windows anywhere, including boundaries. This fixture allowance covers a short transition/start/end ambiguity, not seconds of dropout. It applies to the generated voiced harmonic source at VAD zero; real speech silence, intended-VAD behavior, and microphone synchronization need a different acceptance contract, not silent exemptions here.

The profile observes feeder exceptions and publisher exit codes during normal and recovery sampling and before Stop/fault injection. Unexpected errors or any publisher exit (even zero before its finite source budget should finish) fail the case. Only raw-input closure and publisher exits after an explicitly recorded fault or controller-Stop boundary are expected. Other feeder exceptions still fail. The source receipt records observations and exception classification; historical receipt-only BrokenPipe errors cannot retrospectively establish their timing or cause.

The predeclared envelopes live in `LIMITS` in `tools/media_acceptance.py`. They require a 9–13 second normal file, at least 250 decoded video frames, positive audio packets, at least 95% valid frame identities, seven seconds of source progression, no progression freeze longer than 500 ms, a three-second local scene-age ceiling after startup, and a game A/V offset within 500 ms. Fault recordings can include the additional 20-second detection budget. Their content assertions cover the known pre-fault ten-second prefix. The entire fault file still receives packet and EOF-decode checks. These are first software-fixture envelopes, not supported deployment budgets. Measured values and failures belong to individual run receipts.

Frozen and delayed cases distinguish the independent observer from the Agent's liveness checks. A successful negative case means the observer detects bad content while the actual transport counters still advance. It does not mean the Agent enforces freshness. Disconnect, stall, and worker death must leave Running without automatic output restart. Stop must reconcile cleanup before fresh recovery. Negative cases can explicitly retry one reported incomplete Stop. The receipt retains the unknown outcome and cleanup obligation before that retry. No Start or Prepare is replayed across an unknown outcome.

The receipt binds the source manifest, build command, controller binary hash,
pinned lockfiles, tool hashes, observed output, and cleanup. The launcher
checks the host Git identity and source before and after the run. Inside the
namespace the observer rehashes the source from the read-only file inventory,
so a linked worktree's Git metadata stays outside. `sandbox.exit` and
`sandbox.log` record the namespace command's actual exit. If startup fails
before the sandbox observer records `environment.json`, the launcher log
and exit are the available evidence. Each case records cleanup signals,
reaped exit statuses and independently checked residual resources.
Any changed source, incomplete matrix, unavailable OBS measurement,
failed decode, forced cleanup or incomplete cleanup keeps acceptance
closed. Ordinary `verify` and `verify-virtual` remain separate regression gates.

See [the verification record](verification.md) for actual outcomes. This procedure alone is not a passing result.
