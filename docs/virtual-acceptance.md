# Virtual system acceptance

Virtual acceptance covers control, transport, state transitions, and recovery. Hardware performance is not a prerequisite for developing or testing those properties. A virtual pass does not authorize deployment or establish physical capture safety.

## Run the local software checks

Run the ordinary matrix first.

```sh
nix develop -c cargo run -p splitcast-xtask -- verify
```

Run the extended virtual matrix to include isolated private audio, generated recording, and generated TCP transport.

```sh
nix develop -c cargo run -p splitcast-xtask -- verify-virtual
```

The extended command uses generated inputs and loopback listeners only. It does not use real SSH hosts, capture devices, an existing audio graph, a real OBS instance, or a public output. Both commands preserve every step's command, output, and exit status in a new directory under `artifacts/verification/runs/`. The revision and dirty status describe the source checkout, not an installed operating-system generation.

An unavailable tool, failed step, or missing test is a failure, not a skipped pass. Historical results are not added to the current run.

The additional [real production-media profile](media-acceptance.md) runs separately with `nix develop --no-write-lock-file .#media-test -c cargo run -p splitcast-xtask -- verify-media`. It requires real MediaMTX, patched OBS, private audio, and independent recording inspection. It does not replace either regression matrix or establish capture-Agent, SSH, hardware, or deployment acceptance.

## Software contracts

| Area | Required observation | Limit |
| --- | --- | --- |
| Two-agent control | Actual capture and production agent processes exchange authenticated requests, preserve ownership, stop, and recover through explicit commands. | External services may be stand-ins. This does not prove their implementation. |
| HTTP writes | A slow reader cannot renew one absolute response budget by making intermittent progress. | The bound covers socket writes, not arbitrary filesystem or kernel stalls. |
| RPC cancellation | An expired or cancelled queued command cannot begin effects. A request that already started cannot be described as certainly unexecuted. | A disconnected client may not be detected immediately. Lost responses remain ambiguous. |
| Urgent Stop | Authorized intent is admitted independently of blocked Prepare/Start and ordinary queue capacity. Old queued activation and cancelled Running commits are refused. | HTTP 202 is intent only. An already-entered syscall or transmitted RPC still requires reconciliation. Finite parser slots are not DoS immunity. |
| Server lifecycle | Cancellation stops admission, closes accepted sockets, stops owned workers, and joins connection threads before returning. | No universal hard return bound for uninterruptible kernel I/O or escaped descendants. |
| Generated transport | Generated H.264 and AAC traverse a bounded delayed TCP proxy. Disconnect releases owned resources. A fresh run records and fully decodes both streams. | This is a local software-codec transport trial, not the agent's RTMP and RTSP media path or a real two-host network. |
| Recovery | Failed effects do not silently restart output. Explicit Stop retains unconfirmed cleanup obligations until release. | In-memory recovery is not durable replay or exactly-once remote execution. |

Inspect [the current verification record](verification.md) for executed results. The contracts above are requirements, not implied passes.

The candidate separates ordinary Stop intent from active preparation. The
[control contract](virtual-control-design.md) distinguishes requested,
completed, incomplete, and unknown outcomes. Final-tree acceptance remains
recorded per revision in [PR #2](https://github.com/tenzyu/splitcast/pull/2).
The [review follow-up](evidence/urgent-control-review-20261114.md) covers expired
owner release, pre-dispatch cancellation, ephemeral observation, and replay
limits. Physical cleanup can still wait
for a cooperative boundary or current external deadline. Virtual integration
of the actual agent RTMP and RTSP media path, decoded-frame freshness, and
broader congestion recovery remains open. The separate generated TCP test does
not close that software gap, and hardware testing is not needed to address it.

## Production-only media result, 2026-11-15 JST

S20/E16 supersedes the missing-integration statement above only for the generated production-only profile at `42a64e4`. Actual Agent-owned MediaMTX, RNNoise/private Pulse, and OBS produce independently decoded local recordings. Eleven oracle tests and six cases cover normal output, frozen frames, bounded source backlog, disconnect, stall, worker death, and explicit fresh recovery. Three real-recording mutations reject incorrect A/V content or timestamps. AV01 supersedes the initial phase-fit timing claim. See [the exact receipt](evidence/media-plane-20261115.md) for final regression outcomes and preserved failures. Capture-Agent/SSH, actual network impairment, live freshness enforcement, and microphone alignment remain open. The historical S16 stand-in and detached TCP results keep their original scope.

## Independent physical acceptance

These conditions remain separate from the virtual matrix.

- Direct Scanout behavior with the selected compositor, display mode, and workload.
- GPU, CPU, memory, thermal, and power contention under matched game-only and capture workloads.
- Actual capture-helper privileges, device permissions, and selected source provenance.
- PipeWire and physical audio behavior, microphone isolation, and local gameplay audio independence.
- Real GSR capture and production OBS decode, preview, recording, synchronization, frame age, and bounded media queues.
- Actual host network isolation, SSH policy, cgroup containment, and authorized forced-death cleanup.

Follow [the operational runbook](operations.md) only with separate authorization for those tests. Keep `rustCandidate=false` and empty autostart targets. A virtual pass does not change those gates.
