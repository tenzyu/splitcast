# Operational test runbook

This runbook covers local development checks and separately authorized host tests. Use the capture and production roles when identifying hosts in records.

Current authorization governs physical and external effects. The
[versioned Wiki](memory/session-state.md) records the latest live evidence and
links older, separately scoped trials. Use trial-owned resources and an
explicitly selected game, microphone and playback sink. Bound trial recordings
by duration and file size before Start.

## 1. Record the artifact and scope

Record the Git commit, dirty status, operating system, compiler, dependency lockfile, package path when applicable, command, result, and evidence directory. A source commit does not identify a running operating-system generation. Imported historical results do not count as fresh standalone passes.

Before each test, name its inputs and side effects. Refuse a step that needs unavailable credentials, a display, hardware, privileges, an external publication target, or a service cutover. Mark it BLOCKED or NOT RUN, with the reason. Never substitute a different capture mechanism without changing the contract.

## 2. Run local checks without host activation

```sh
nix develop -c cargo run -p splitcast-xtask -- verify
nix build .#default
nix flake check
```

The verifier runs fmt, workspace/all-target check and Clippy, unit and integration tests, doctests, the no-default-features matrix, the valid all-features check, dependency trees, and repository portability. Each command's output and exit status are retained under `artifacts/verification/`. A failing command makes the verifier fail even if later checks pass.

Hosted CI runs those checks on ephemeral runners. Signed GitHub Releases can
provide the same pinned package outputs without builds. Follow the exact source,
cache URL, keys and consumer result in [the CI/cache record](https://github.com/tenzyu/splitcast/blob/archive/pre-dreaming-20261117/docs/evidence/ci-nix-cache-20261114.md),
not a moving release tag. Publishing a cache never enables runtime or autostart.

`nix develop -c cargo run -p splitcast-xtask -- verify-virtual` also runs the isolated private-audio, generated-recording, and loopback generated-transport tests. See [virtual system acceptance](virtual-acceptance.md) for the software contracts and separate physical gates. This command does not authorize real SSH, hardware capture, an existing OBS instance, or public streaming.

For the additional real production software path, follow [real local media acceptance](media-acceptance.md). Its opt-in `media-test` shell uses a device-free private namespace, generated publishers, actual production services, and local OBS recordings. Do not invoke the underlying Python orchestrator outside its namespace launcher.

Ordinary tests exercise authenticated local sockets, protocol rejection, private output selection, peer ownership, worker failures, and timeouts with stand-in workers. They must not contact a real SSH peer or open capture devices.

Run isolated optional local components separately:

```sh
nix develop -c cargo test -p splitcast-rs-core private_audio_dies --locked --offline -- --ignored --nocapture
nix develop -c cargo test -p splitcast-rs-core --test local_synthetic --locked --offline -- --ignored --nocapture
```

The audio test starts its own Pulse server containing a null sink, checks readiness, stops the owned process, and verifies connection refusal and socket cleanup. It does not modify the user's audio graph. The generated-media test creates a two-second 320×180/15 fps video and sine wave, records H.264/AAC, verifies streams and duration, and fully decodes the file. It reads no physical input and opens no network listener. Neither test establishes game capture, remote production, or physical microphone safety.

Inspect available supply-chain checks separately:

```sh
nix develop -c cargo audit
nix develop -c cargo deny check
```

Audit database availability, license policy failures, or unavailable tools are results to record. Do not suppress them or infer that a clean build is a security audit. Miri requires a compatible nightly and cannot generally execute these native socket/process/FFI integrations. Record an unrun Miri or fuzzing check explicitly.

## 3. Validate configuration and deployment gates

```sh
result/bin/splitcast selfcheck
result/bin/splitcast validate --config packages/splitcast-rs/example.toml
result/bin/splitcast validate --config packages/splitcast-rs/example-production.toml
result/bin/splitcast doctor --offline --config packages/splitcast-rs/example.toml
```

Selfcheck leaves media idle. Both examples validate, and offline Doctor reports
that the example's unselected capture setup requires configuration. Provision
secrets outside Git and outside the Nix store. Configure the capture and
production roles through SSH aliases and role settings. Share transport
geometry, ports and the speech expiry contract. Verify media independently of
the control endpoint.

Evaluate the consumer's configuration before any activation. Confirm capture
remains disabled unless separately approved, `rustCandidate=false`, empty
`WantedBy`, migration-gated `ExecStart`, `KillMode=control-group`,
`SendSIGKILL=true`, a 25-second service stop timeout, and private state
settings. The enabled unit's `ExecStop` calls the authenticated Agent Stop
before systemd signals the cgroup. Optional display and stock-OBS firewall
modules remain disabled by default. [Nix integration](nix-integration.md)

## 4. Authorized synthetic two-host transport

Do not run this section without explicit authorization for the selected hosts, SSH identity, user units, and finite generated-media workload. Verify the remote host key independently. The test requires `ssh`, `timeout`, `systemd-run`, `systemctl`, FFmpeg and FFprobe locally, plus a reachable remote user service manager and a separately installed remote FFmpeg.

Set these variables in the test shell, with the identity and tool path belonging to the authorized environment:

```sh
export SPLITCAST_SYNTHETIC_AUTHORIZED=1
export SPLITCAST_SYNTHETIC_SOURCE_HOST=capture-host
export SPLITCAST_SYNTHETIC_IDENTITY="$HOME/.ssh/splitcast-test"
export SPLITCAST_SYNTHETIC_REMOTE_FFMPEG=/usr/bin/ffmpeg
export SPLITCAST_SYNTHETIC_SECONDS=10
export SPLITCAST_EVIDENCE_DIR="$PWD/artifacts/synthetic-ssh"
nix develop -c cargo test -p splitcast-rs-core --test synthetic_ssh synthetic_av_over_encrypted_ssh --locked --offline -- --ignored --nocapture
```

The environment variable is an explicit execution gate, not proof that the operator actually granted authorization. Local tool paths can be overridden with `SPLITCAST_FFMPEG` and `SPLITCAST_FFPROBE`. The remote FFmpeg path must be absolute and pass the shell-safe path validation. The example path is not assumed to exist on a NixOS host; supply that host's actual authorized executable.

Only 10-second and separately approved 600-second durations are accepted. Each source and sink unit is limited to one CPU equivalent, 512 MiB, 660 seconds runtime, five seconds stop time, and control-group killing. These are configured limits, not measured utilization. Verify both units and owned descendants are gone after every run, including a failed assertion. Do not interpret a cleanup warning as a clean stop.

For a separately authorized disconnect check, use the short duration with `SPLITCAST_SYNTHETIC_DISCONNECT=1`. Inspect independent source/sink teardown evidence. An interrupted output is not required to decode, so that run proves cleanup only. The complete run must contain both expected codecs and packets, the expected duration, and a successful full-file decode.

This harness is not the Rust agent, GSR, OBS, physical audio, camera, or gameplay. Its new portable parameterization has not automatically inherited the old host-specific live results.

## 5. Live candidate acceptance, separately authorized

Before capture, record local audio routes, input mappings, renderer, resolution, focus, display/scanout behavior, GPU/CPU state, and the exact workload. Do not alter the game or local playback to make the candidate pass. Verify the narrow privileged capture boundary and selected device provenance. A name filter is not a hardware audit.

Start only the reviewed opt-in candidate and its owned workers. Check actual decoded moving pixels, preview, recording, stream types, processed microphone output, synchronization, frame age, bounded queues, reconnect behavior, and local gameplay. Request failures and partial startup must stop owned effects. Lost Prepare replies must receive owner-scoped rollback. An unconfirmed remote Stop must retain the obligation rather than report Idle. Confirm process, socket, file, connection, and cgroup cleanup independently after disconnect, child failure, SIGTERM, and an explicitly authorized forced-controller-death trial.

Restart the same osu! replay segment from its beginning for each OFF/ON/OFF
run. Preserve versions, settings, input route, thermal state, focus, Direct
Scanout, and game-only audio routing. Keep the authorized renderer without
Vulkan/Zink. Record sample counts, frame-time distributions, uncertainty, and
unmeasured input or audio dimensions. The gameplay p99 regression limit is
1 ms and the late production render limit after warmup is 1%. Judge both
against the matched workload.

Decode the output independently and locate actual game and camera changes,
an identifiable physical SoloCast/RNNoise signal separated from game audio,
and their alignment on the decoded media timeline. Gather the physical sound
and replay restarts at the agreed boundaries. Byte counts, decoded frame
counts, and scene geometry show progress or configuration. Camera freshness
requires content observations and cross-host latency requires a measured clock
relationship or a shared timing reference.

Finite speech return has a [software acceptance receipt and rollback procedure](evidence/audio-return.md). The live EventSub and media trial results are in the versioned Wiki. Full crash and performance acceptance remains open. They cannot be accepted by turning off their assertions. Public broadcast requires separate authorization even after every local gate passes.

## Native PoC startup

Build and stage the same reviewed binary on both hosts with the pinned flake.
Provision mode-0600 bearer and OBS password files. Verify the peer host key and
use a restricted SSH identity that permits only the configured loopback
forwards. `peer.ssh_config` can select a trial-specific OpenSSH configuration.
Start the production Agent, then the capture Agent in their own bounded
systemd cgroups. Give the production Agent a private display and recording
directory. Use `obs.video_mode = "hardware"` with the supported isolated
NVIDIA display and verify the actual GL renderer, NVENC encoder and decoder
path during the trial. `software` selects CPU decode and x264 for isolated
diagnostics. Normal Prepare starts MediaMTX and leaves capture and OBS idle.

Set `obs.separate_audio_tracks = true` on production for a private MKV with
the mix on track 1, game audio on track 2, and processed mic on track 3.
Twitch uses the mixed track. Decode the independent tracks to check that the
selected physical sound reaches the processed mic track and is absent from
the game track.

```sh
splitcast --config ./capture.toml prepare
splitcast --config ./capture.toml start --output recording --duration-seconds 20
splitcast --config ./capture.toml status
splitcast --config ./capture.toml stop
```

For the KMS trial, build the exact helper through NixOS
`programs.gpu-screen-recorder` and restrict it to the capture user in a
time-limited system unit. Record the active generation and current wrapper
link before activation and verify both again after release. The trial
used the NixOS-generated wrapper in a separate runtime directory under
`/run/wrappers`.

Keep the capture window visible on its authorized Direct Scanout output.
Stop if scanout or the selected local game-audio links are lost. The optional
`--duration-seconds` value runs from 1 to 28800 and schedules Stop from
production as output activation begins. Recording and bandwidth test may
omit it for operator-managed use. The capture Agent renews the production
ownership lease while connected. Lease expiry revokes activation and starts
cleanup. An output failure requires operator recovery and a fresh Start.
Set the trial production unit's file size limit and account for each
recording before another attempt. Independently decode each output and
retain PTS, content and failure receipts. Remove trial-owned recordings
after examination.

For speech without capture, start production-local VOICEVOX and use
`prepare --speech-only` on capture. Set `tts.enabled` and the same expiry
on both Agents. The 25-second trial budget passed the earlier
physical utterance that was clipped at 10 seconds. Configure a capture
`tts_sink` and optionally `tts_volume_percent` from 0 to 100. Apply a new
gain on the next Agent launch after stopping the prior session.

To select live chat, set `tts.eventsub = true` on production and set
`tts.credentials_file` to a private JSON file containing `client_id`,
`access_token`, `broadcaster_user_id` and `reader_user_id`. Use an existing
suitable user token or request `user:read:chat` once through the official
device flow. The operator completes the consent step. Keep the token
outside the Nix store. The Agent validates identity and creates the
specific WSS subscription. Transient failures retry with a cancellation-aware
delay from 1 to 30 seconds. Invalid credentials, an identity mismatch or an
explicit revocation of the active subscription require operator recovery.
Use `tts` to inspect subscription state, reconnects, duplicates, queue drops,
admissions and synthesis. Within one prepared session the Agent remembers up
to the most recent 4,096 distinct chat message IDs for ten minutes from their
first observation across reconnects. A message dropped from a full queue
still occupies its deduplication entry. Beyond either retention bound, or
after a new Prepare, a replay can be admitted again. The Agent does not
backfill chat missed during a disconnect.
[Twitch chat authorization](https://dev.twitch.tv/docs/eventsub/reference/chat#channel-chat-message) ·
[Twitch device flow](https://dev.twitch.tv/docs/authentication/getting-tokens-oauth/#device-code-grant-flow)

### Camera and controls

Use `splitcast --config FILE devices` to discover stable V4L2 paths.
To enable the camera, configure the same location and video geometry on both
Agents. Keep `device` on the role that opens it. Choose a format and frame rate
reported by that device. The camera appears in the upper-right OBS inset.

```toml
[camera]
location = "capture"
device = "/dev/v4l/by-id/usb-example-video-index0"
width = 320
height = 180
fps = 15
input_format = "yuyv422"
encoder = "libx264"
```

For native H.264 select `input_format = "h264"` and `encoder = "copy"`. When
selecting `h264_vaapi`, specify the corresponding `render_device` on the
camera host. Omit the camera section on both roles when the session has no
camera. Speech-only Prepare leaves camera and media workers idle.

Open `http://127.0.0.1:17870/` for the example capture Agent and enter its
local control bearer. The browser holds the bearer in the current page only.
Use the Web preview to examine the actual OBS composition. Run
`splitcast --config FILE tui` for the terminal menu. Stop is available in
both controls while an ordinary command is in flight. From the CLI, use a
second invocation when the first is pending.

The clients send the current Agent `start_id` as `session_id` with Start.
The Agent rejects a stale ID if Stop or another transition has invalidated it.
A new public output requires a prepared media session in `ready`.

After preparing speech, use `splitcast --config FILE say "Test speech"`
or Speak in the Web or TUI control. Each client obtains `next_id` from
authenticated `tts` status and submits the text once with that ID. CLI also
accepts an explicit `--id`. Concurrent speech can invalidate the ID. If
admission is rejected, inspect `tts` status before deciding whether to
submit a new utterance. A lost response leaves speech admission unknown.

### Broadcast overlay

On production, install the Noto Sans CJK JP font and set `obs.overlay_text`
to one line of up to 80 characters, for example `テスト中だよー`. The Agent
adds the text to the upper-left of the OBS scene during Start. Inspect it in
Web preview during a private recording before selecting public output.

### Twitch output

Put the channel stream key in a private file owned by the production Agent
user. Use a runtime path, with mode 0600, outside the Nix store. Configure
the official RTMPS ingest host for the region on production.

```toml
[twitch]
ingest_host = "apn10.contribute.live-video.net"
key_file = "~/.config/splitcast/stream-key"
allow_public = false
```

Choose `twitch-test` to send to Twitch Inspector in bandwidth-test mode.
For the sustained acceptance trial, observe at least five healthy minutes
after Running, then confirm Stop. The example watchdog leaves time to observe
the full window.

```sh
splitcast --config ./capture.toml start --output twitch-test --duration-seconds 360
```

After separate authorization for the selected public channel and trial
window, set `allow_public = true` on production and relaunch the production
Agent with that configuration. Prepare the media session and wait for
`ready`. At Start, submit `confirm_public = true`, the agreed duration
and the current `session_id`. The CLI uses `--confirm-public` and
`--duration-seconds` and obtains the session ID from Status. Web prompts
after a duration is entered. TUI prompts for the duration and requires typing
`LIVE`. Use an independent viewer to verify actual playback during the
authorized window.

```sh
splitcast --config ./capture.toml prepare
splitcast --config ./capture.toml start --output twitch-live --duration-seconds 60 --confirm-public
```

The public duration schedules Stop and fences further activation. Actual
cutoff still depends on cleanup, which is why the trial also needs a bounded
service and a viewer-side stop check. Twitch chat authorization and the
stream key are independent credentials. Keep the OBS profile private and use
a service with core dumps disabled.

After Stop, confirm completed cleanup, including release of the private
OBS profile. If the enclosing service had to be terminated, let its
`ExecStopPost` run after the cgroup is empty, or run
`splitcast --config FILE cleanup` after independently verifying worker
termination. Dispose of the trial key file after verification.
[Twitch broadcast documentation](https://dev.twitch.tv/docs/video-broadcast/) ·
[Twitch ingest directory](https://ingest.twitch.tv/ingests)

## Stop and rollback

Use the candidate's explicit Stop, inspect status and diagnostics, then stop only its enclosing user unit if needed. Do not kill by process name, stop existing OBS, change the default sink, restart the game, or remove unrelated files. If remote release cannot be confirmed, retain the failed/stopping result and inspect the remote ownership lease and service cgroup.

During in-flight work, `202` with `stop: "requested"` confirms intent
admission. The clients observe status until they see `stop: "completed"` and
Idle for that `stop_id`, or report cleanup uncertainty. An incomplete Stop
requires a fresh explicit retry after the fault is addressed. A timeout or lost
response leaves the outcome unknown. Inspect status and coordinate clients
before issuing a new Start or Prepare. Capture retains
`remote_release_pending` until production confirms completion for the held
owner. A proof must match the owner even when the local child list is empty.

On production, inspect `obs_shutdown` after Stop. An acknowledged inactive
output and an OBS exit code of zero are separate observations. The Agent
tries to stop the output over WebSocket, asks the owned OBS process to close
via SIGINT and uses bounded group cleanup if needed. Record the actual exit
code or signal, including abnormal exits after file finalization. For
recording acceptance, independently decode the final file. Keep private
OBS logs or crash diagnostics in a trial-owned location and sanitize them
before sharing. Keep stream keys, OBS passwords and bearer tokens out of
receipts.

During pending cleanup, authenticated GET status remains available from control
metadata, with `children: null` and unknown resource observation. After the
observation budget, cleanup can be explicitly unknown while intent remains
retained. A status HTTP 200 acknowledges an observation, not completed Stop.
The production bearer also grants unowned emergency Stop. A peer holding that
bearer is not a separately isolated principal merely because it usually sends
`_owner`.

An expired matching owner retains Stop authority over its held effects.
Activation and lease renewal require the current owner with a valid lease.
After removing a cleanup fault, retry with the same `_owner` and require
its matching completed/Idle proof.

Coordinate clients before the next Prepare. A new session can overwrite the
prior Stop observation. If the `stop_id` is missing or has changed before
completion is observed, report the earlier outcome as unknown. Pause competing
commands and verify current ownership before sending a fresh Stop. An
admitted Prepare from Idle can create new effects, including a delayed
Prepare from an earlier session. Quiesce request sources and verify held
resources before authorizing another session.

The admission measurements cover ordinary queue pressure after a request can be
parsed. Eight slow parser/connection clients can still exclude emergency HTTP
control. Keep access restricted to trusted clients and retain a separately
authorized enclosing-service recovery path. No universal HTTP deadline or
deployment containment is established by the local fixture measurements.

For a code rollback, check out a known commit in a separate checkout and build it without overwriting local work. For consumer integration rollback, restore the previously reviewed flake input and settings in a separate reviewable change. Do not activate an older operating-system generation or retained Python implementation without separate approval. A Git rollback is not a deployment rollback.
