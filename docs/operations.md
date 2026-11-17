# Operational test runbook

This runbook separates local development checks from tests that require current operator authorization. It is not a command to enable a production service. Capture and production are roles, not fixed machine identities.

The [initial PoC checkpoint](evidence/poc-20261117.md) records S24's narrower
trial policy. Its recordings are temporary-only, at most 30 seconds each and
500 MB aggregate, inspected then deleted only by their trial owner. Do not run
the existing retained-media profile unchanged under that permission. Read-only
SSH is not a worker-launch channel. The [fixture playback command](poc-speech.md)
does not connect Twitch or authorize physical listening by itself.

[S25's convergence checkpoint](evidence/poc-convergence-20261117.md) records the
later Twitch authorization. WSS, configured-channel chat subscription, necessary
Helix calls and existing suitable OAuth use are approved. New OAuth and scope
changes are not. Missing account inputs stop that operation. Capture SSH still
permits metadata investigation only, not worker launch or helper provisioning.

## 1. Record the artifact and scope

Record the Git commit, dirty status, operating system, compiler, dependency lockfile, package path when applicable, command, result, and evidence directory. A source commit does not identify a running operating-system generation. Imported historical results do not count as fresh standalone passes.

Before each test, name its inputs and side effects. Refuse a step that needs unavailable credentials, a display, hardware, privileges, an external publication target, or a service cutover. Mark it BLOCKED or NOT RUN, with the reason. Never substitute a different capture mechanism without changing the contract.

## 2. Run local checks without host activation

```sh
nix develop -c cargo run -p splitcast-xtask -- verify
nix build .#default .#splitcast-web
nix flake check
```

The verifier runs fmt, workspace/all-target check and Clippy, unit and integration tests, doctests, the no-default-features matrix, the valid all-features check, the WASM target check, dependency trees, and repository portability. Each command's output and exit status are retained under `artifacts/verification/`. A failing command makes the verifier fail even if later checks pass.

Hosted CI runs those checks on ephemeral runners. Signed GitHub Releases can
provide the same pinned package outputs without builds. Follow the exact source,
cache URL, keys and consumer result in [the CI/cache record](https://github.com/tenzyu/splitcast/blob/archive/pre-dreaming-20261117/docs/evidence/ci-nix-cache-20261114.md),
not a moving release tag. Publishing a cache never enables runtime or autostart.

`nix develop -c cargo run -p splitcast-xtask -- verify-virtual` also runs the isolated private-audio, generated-recording, and loopback generated-transport tests. See [virtual system acceptance](virtual-acceptance.md) for the software contracts and separate physical gates. This command does not authorize real SSH, hardware capture, an existing OBS instance, or public streaming.

For the additional real production software path, follow [real local media acceptance](media-acceptance.md). Its opt-in `media-test` shell uses a device-free private namespace, generated publishers, actual production services, and local OBS recordings. Do not invoke the underlying Python orchestrator outside its namespace launcher.

Ordinary tests exercise authenticated local sockets, protocol rejection, terminal input, output selection, peer ownership, worker failures, and timeouts with stand-in workers. They must not contact a real SSH peer or open capture devices.

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
result/bin/splitcast doctor --offline --config packages/splitcast-rs/example.toml
```

Selfcheck must not start media. The example must validate but Doctor must not report an unselected capture setup ready. Provision secrets outside Git and outside the Nix store. Configure capture host and production host through SSH aliases and role settings. Share the complete camera contract, transport geometry, routes, and ports. A compatible status endpoint alone is not media readiness.

Evaluate the consumer's configuration before any activation. Confirm capture remains disabled unless separately approved, `rustCandidate=false`, empty `WantedBy`, migration-gated `ExecStart`, `KillMode=control-group`, `SendSIGKILL=true`, a finite stop timeout, and private state settings. Optional display and stock-OBS firewall modules remain disabled by default. Their evaluation is not permission to install, enable, or activate them.

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

Start only the reviewed opt-in candidate and its owned workers. Check actual decoded moving pixels, preview, recording, stream types, processed microphone output, selected camera, synchronization, frame age, bounded queues, reconnect behavior, and local gameplay. Request failures and partial startup must stop owned effects. Lost Prepare replies must receive owner-scoped rollback. An unconfirmed remote Stop must retain the obligation rather than report Idle. Confirm process, socket, file, connection, and cgroup cleanup independently after disconnect, child failure, SIGTERM, and an explicitly authorized forced-controller-death trial.

Measure a comparable game-only baseline before claiming a performance improvement. Preserve replay/workload, versions, settings, thermal state, and focus, then repeat A/B/A trials. Record distributions rather than a single average. Reject any claimed improvement when inputs differ, work is missing, errors occur, or gameplay exceeds the operator's predeclared regression threshold. No baseline or performance gain is claimed by the extraction tests.

Finite speech return has a [software acceptance receipt and rollback procedure](evidence/audio-return.md). EventSub, shared selection transactions, full crash recovery and role-specific backend separation are unfinished. They cannot be accepted by turning off their assertions. Public broadcast requires separate authorization even after every local gate passes.

## Stop and rollback

Use the candidate's explicit Stop, inspect status and diagnostics, then stop only its enclosing user unit if needed. Do not kill by process name, stop existing OBS, change the default sink, restart the game, or remove unrelated files. If remote release cannot be confirmed, retain the failed/stopping result and inspect the remote ownership lease and service cgroup.

During in-flight work, `202` with `stop: "requested"` confirms intent admission,
not resource release. Wait for `200` with `stop: "completed"`, or inspect status
and repeat Stop to retry incomplete cleanup. Status exposes
`remote_release_pending`; do not treat an empty local child list as remote
completion. A timeout or lost response remains unknown. Never blindly retry
Start or Prepare across that uncertainty. A remote Stop completion is bound
to the held owner. Bare Idle or another owner's completion is not release proof.

During pending cleanup, authenticated GET status remains available from control
metadata, with `children: null` and unknown resource observation. After the
observation budget, cleanup can be explicitly unknown while intent remains
retained. A status HTTP 200 acknowledges an observation, not completed Stop.
The production bearer also grants unowned emergency Stop. A peer holding that
bearer is not a separately isolated principal merely because it usually sends
`_owner`.

On the PR #2 candidate, an expired matching owner can still retry Stop for held
effects. It cannot renew the expired lease or activate output. A wrong or older
owner cannot stop a newer session. After removing a cleanup fault, retry with
the same `_owner` and require its matching completed/Idle proof.

Coordinate clients before the next Prepare. Stop observation is ephemeral and
has no per-intent ID. If a new session overwrites completion before you observe
it, treat the earlier result as unknown. Do not use another owner's completion
or an unowned status snapshot as proof. Pause competing commands and reconcile
the current ownership explicitly rather than blindly stopping a newer session.
Never replay an uncertain Prepare. A later Prepare admitted from Idle can create
new effects; completed Stop is not a cross-host replay fence. Quiesce request
sources and verify held resources before authorizing another session.

The admission measurements cover ordinary queue pressure after a request can be
parsed. Eight slow parser/connection clients can still exclude emergency HTTP
control. Keep access restricted to trusted clients and retain a separately
authorized enclosing-service recovery path. No universal HTTP deadline or
deployment containment is established by the local fixture measurements.

For a code rollback, check out a known commit in a separate checkout and build it without overwriting local work. For consumer integration rollback, restore the previously reviewed flake input and settings in a separate reviewable change. Do not activate an older operating-system generation or retained Python implementation without separate approval. A Git rollback is not a deployment rollback.
