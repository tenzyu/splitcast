# PoC convergence checkpoint

Recorded 2026-11-17. Overall PoC completion remains incomplete. The current
blockers are missing Twitch account inputs and implementation, a capture-worker
execution route, and specific capture dependencies. They are not a general lack
of permission to perform PoC tests.

## Confirmed paths and source

- Local fixture WAV validation and finite PipeWire playback are committed in
  `3c2f9136833bac5b3aa356aaf7607538df8ce555`. That commit was already present
  when this continuation reached staging. The continuation did not create it,
  rewrite it, or publish it. Its 15 paths contain implementation, inline tests,
  references and Wiki updates, not private artifacts.
- Retained receipts agree with the tested playback source and binary hashes.
  They show private null-sink playback, missing-target rejection without fallback,
  and SIGTERM cleanup. They do not show physical listening.
- Earlier generated GPU receipts show 15 real NVENC frames and 15 CUVID/CUDA
  decoded frames. The real local VOICEVOX receipt shows a 46,124-byte mono
  24-kHz PCM16 WAV and successful independent decode. This continuation checked
  those primary receipts. It did not rerun the probes or promote fixed text to
  a real Twitch event.
- Private source, configuration and evidence remain under
  `artifacts/poc/20261117-converge/`. Secret values, machine mappings and raw
  environment data are excluded from commits. No new receiver, UI, management
  service, dependency or media framework was added during discovery.

The [initial checkpoint](poc-20261117.md) retains its earlier uncommitted-source
and authorization snapshot. This page supersedes its current-status statements,
not its historical results. The runtime gate remains false and autostart targets
remain empty. This continuation ran no push, fetch, pull, PR publication, merge,
deployment or operating-system activation.

## Twitch authorization is sufficient, but the selected account inputs are absent

The operator now permits production-host connections to
`wss://eventsub.wss.twitch.tv/ws`, `channel.chat.message` v1 for the channel
identified by existing configuration, necessary Helix subscription calls, and
existing suitable OAuth credentials. These permissions do not need to be asked
again. New OAuth acquisition, scope additions, chat posting and public streaming
remain forbidden.

The current production TOML names an OAuth JSON path, but that file does not
exist. The TOML has no channel or broadcaster selector. Selected Twitch, OnAir,
chat-client and OBS configuration locations on production contain no receiver or
account configuration. No matching receiver executable, process or user service
was found in the inspected scope.

Read-only capture-host discovery finds OBS profiles with stream keys, including
one Twitch service selection. Their profile, global and user INI files expose
no Twitch OAuth section or account selector. Stream keys authorize publication,
not `channel.chat.message`, and were not copied or used. This corrects the
broader hypothesis that no Twitch-related asset exists anywhere inspected.
It does not supply an EventSub credential or selected channel.

The OAuth scope is **unverified because a usable token is absent**, not a proven
scope rejection by Twitch. No OAuth endpoint, EventSub connection or Helix API
request ran. No subscription was created, changed or deleted. No new Twitch
client was implemented while the receiver and account inputs remain unselected.
SplitCast's normal Agent still refuses unfinished EventSub and speech delivery.

The next input is the selected channel and the path to an existing appropriate
OAuth configuration, or an existing receiver's supported local event output.
Do not paste tokens into chat or copy them into the repository. If no existing
credential is available, new OAuth acquisition needs separate authorization.

## Exact capture dependencies

Read-only inventory goes beyond PATH discovery:

- The capture host's checked-out feature selection disables SplitCast capture
  in both system and user layers. This is source configuration, not a fresh
  evaluation of the running generation. Actual user-service metadata reports
  `splitcast-capture.service` not found and inactive. No preconfigured SplitCast
  launcher or capture TOML is present in the inspected deployment locations.
- GSR and its KMS helper are absent from the system profile, user profile,
  wrapper directory, and matching cached package roots. The current SSH process
  has no effective capabilities. DRM nodes are accessible, but device access
  alone does not grant the KMS framebuffer capability required by this route.
- `nix/modules/nixos.nix` connects the capture role to
  `programs.gpu-screen-recorder.enable`. The inspected Nixpkgs module installs
  GSR and a root-owned `gsr-kms-server` wrapper with `cap_sys_admin+ep`. That
  missing executable and narrow privilege boundary are concrete prerequisites.
  This finding is not permission to enable the module, install a helper, grant
  capabilities, run sudo or switch a generation.
- FFmpeg is **not entirely absent**. A cached FFmpeg headless binary exists.
  Its static listings advertise `kmsgrab`, `h264_vaapi`, AAC and Opus, but no
  Pulse or PipeWire demuxer. The current `media::mic` command uses `-f pulse`,
  so that cached binary cannot satisfy the current microphone input contract.
  Codec listing does not establish successful hardware encoding or capture.
- Existing `pw-record` can be considered for raw microphone acquisition with
  production-side processing. That avoids requiring a new capture-side Pulse
  FFmpeg build, but the path has not been implemented or tested.
- The desktop portal and Hyprland portal services are active. OBS and its
  PipeWire plugin exist. GSR's locally inspected manual describes a portal mode,
  but GSR itself is absent on capture. These are alternative backend candidates,
  not a demonstrated capture path. No portal session, consent dialog, OBS launch,
  renderer change, or implicit backend substitution occurred.

The next game-capture condition is an operator-approved usable GSR executable
and narrow helper route, or an explicit choice to evaluate an existing portal
backend against the same gameplay and privacy requirements. All-frame baseline
and Direct Scanout effects must be measured, not inferred from backend presence.

## Remaining paths by blocker class

An item may have more than one blocker. The table separates code from execution
authority rather than attributing every unfinished path to permissions.

| Path | Implementation | Execution, dependency or authority | Verification |
| --- | --- | --- | --- |
| Real Twitch notification to production synthesis | EventSub admission and its synthesis connection are absent. Existing synthesis component and VOICEVOX exist. | Connection and subscription approved. Channel and suitable existing OAuth configuration are missing in the inspected scope. | Real reception not run. No failed Twitch call or proven scope rejection. |
| Production audio to capture playback | Local finite player exists. Authenticated cross-host return is absent. | Native player is present on capture, but read-only SSH is not a launch route. Reviewed SplitCast binary is not installed there. | Private local fixture checks pass. Capture-host playback not run. |
| Game video and game-only audio to production | GSR command and production receive/composition code exist. | GSR executable and authorized KMS helper absent. No authorized capture-worker launch route. | Physical acquisition and transfer not run. No capture-enabled performance result. |
| Microphone to production | Current Pulse/Opus command and production RNNoise path exist. | Cached FFmpeg lacks Pulse input. Native `pw-record` is an untested alternative. No authorized worker launch route. | Physical microphone acquisition and transfer not run. |
| Fixed-workload OFF/ON comparison | No selected all-frame observation route or fixed replay/input workload. | Prior sampled OFF metadata is insufficient. | Frametime distributions, live media age, fps and event-to-playback delay not measured. No acceptance margins proposed. |
| Integrated failure and temporary recording lifecycle | Current retained-media profile does not implement the trial's new recording policy. | Recordings must remain temporary, at most 30 seconds each and 500,000,000 bytes aggregate. | Integrated faults and recording acceptance not run. This continuation creates no recordings or media workers. |
| Ordinary software verification | Existing local test suite runs. | No external or physical prerequisite. | One full run failed. A focused recheck and a later full run passed at their recorded sources. Failure is retained below. |

## Narrow capture playback execution request

No new capture execution is performed by this document. After production event
admission and synthesis are ready, the first proposed extension is one finite
returned-audio trial, not arbitrary SSH commands:

- Connection: the existing-key, host-key-verified SSH route from production to
  capture, extended only to the fixed player launch, exact-unit stop and metadata
  checks below. No sudo, file transfer, daemon enabling or persistent settings.
- Executables: the capture host's existing `systemd-run`, `pw-play` and
  `systemctl`. Resolve their existing system-profile binaries, not a downloaded
  or newly installed copy.
- Input: one validated mono PCM16 utterance at 24 kHz, at most two seconds and
  96,000 PCM bytes, over stdin. No chat text or token enters argv. No audio file,
  microphone, screen capture, default-route change or network listener on capture.
- Resources: the already identified playback node, a transient owned user cgroup,
  one CPU quarter, 128 MiB and at most 16 tasks. Node selection is explicit.
- Runtime: at most 10 seconds before systemd termination, plus up to three
  seconds stop grace. The launcher has a separate finite wait budget.
- Stop: normal stdin EOF, then observe the exact unit. On failure, signal or
  disconnection, stop only that unit and verify inactive/not-found, PID zero,
  released player and graph links. Stop failure remains a failed cleanup receipt.

The proposed fixed command shape uses reviewed node and per-trial unit names:

```sh
systemd-run --user --wait --pipe --collect --unit="$TRIAL_UNIT" \
  -p RuntimeMaxSec=10s -p TimeoutStopSec=3s -p KillMode=control-group \
  -p CPUQuota=25% -p MemoryMax=128M -p TasksMax=16 -- \
  pw-play --raw --format=s16 --rate=24000 --channels=1 \
  --target="$TARGET_NODE" \
  '--properties={ node.dont-fallback = true node.dont-reconnect = true application.name = splitcast-tts }' -
systemctl --user stop "$TRIAL_UNIT.service"
systemctl --user show "$TRIAL_UNIT.service" -p LoadState -p ActiveState -p MainPID
```

`TRIAL_UNIT` is generated under `splitcast-poc-return-` with safe characters.
The native properties were tested in isolation, not on this physical session.
This request covers returned playback only. Game and microphone launch commands
need their own resolved dependencies and finite execution specification.

## Verification and changed source identities

Commands run directly in the reviewed Nix/direnv environment:

| Exact command | Observed result |
| --- | --- |
| `python3 artifacts/poc/20261117-converge/audit.py` | Exit 0. Retained 165-test receipts, playback source and binary hashes, CLI exits, GPU and VOICEVOX observations agree. Private artifacts are ignored. Scoped candidate privacy scan finds no secret or host-specific data. No media rerun. |
| `cargo run -p splitcast-xtask -- verify` | First run exits 1, `artifacts/verification/runs/1794912076155129812/`, based on `a5e7a22` plus then-uncommitted work. Default selection passes 165 tests. Three no-default-feature two-agent cases fail with `Connection reset by peer` and empty responses. All logs remain. |
| `cargo test -p splitcast-rs-core --test two_agents --no-default-features --locked --offline -- --nocapture` | Exit 0, eight tests and one ignored child entry. This focused recheck does not diagnose or repair the earlier reset. |
| `cargo run -p splitcast-xtask -- verify` | Later run exits 0, all eleven stages, 165 passing tests in each feature selection. Run `artifacts/verification/runs/1794921208376624165/` identifies clean `3c2f913`, not the first run's source. |
| `python3 artifacts/poc/20261117-converge/discover.py` | Exit 0. Selected deployment structure and credential presence only. No token values or external calls. |
| `python3 artifacts/poc/20261117-converge/run-capture-inventory.py` | Read-only SSH exits 0. Backend, service, capability and static codec/input facts above. First inventory overselected source host files because it matched a username in the path. That receipt is retained. Directory-component filtering corrects the source scope. No host setting changed. |
| `python3 artifacts/poc/20261117-converge/run-capture-inventory.py capture-obs-metadata.py` | Read-only SSH exits 0. Profile and global authentication field-presence metadata. Stream-key values were neither emitted nor reused as OAuth. |
| `python3 artifacts/poc/20261117-converge/final-check.py` | Exit 0. Latest verifier identity and counts agree. Playback source hashes match retained native receipts. Documentation links and scoped privacy scan pass. No remaining owned trial units or matching two-agent fixture processes are observed. Production remains inactive and migration-gated. |

The tree changed outside this continuation while work was underway. Reflog shows
the prior pull of `ec38ee3`, then local commit `3c2f913`. The former changes
test-only listener-release assertions to distinguish closed sockets from
`TIME_WAIT`. Neither change is attributed to this continuation, and neither is
claimed to fix the observed prepare reset. The two whole-suite runs therefore
are not same-source reproductions. No further automatic reset retries are run
under the three-attempt limit, counting the focused recheck.

No worker, listener, subscription or recording was created by this continuation
beyond ordinary device-free test fixtures. Native-player and GPU/VOICEVOX cleanup
claims remain tied to the earlier receipts. Current process and unit metadata
checks are recorded separately in the private final handoff.

## Next start condition

Provide the selected channel and an existing OAuth configuration path or a
supported existing local receiver output. With those inputs, check suitability
without logging secrets, then complete real reception to VOICEVOX before capture
playback. If the inputs remain absent, that operation remains stopped. Do not
replace it with a mock PASS, an OAuth acquisition, or a duplicate generic client.
