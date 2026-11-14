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

## Software contracts

| Area | Required observation | Limit |
| --- | --- | --- |
| Two-agent control | Actual capture and production agent processes exchange authenticated requests, preserve ownership, stop, and recover through explicit commands. | External services may be stand-ins. This does not prove their implementation. |
| HTTP writes | A slow reader cannot renew one absolute response budget by making intermittent progress. | The bound covers socket writes, not arbitrary filesystem or kernel stalls. |
| RPC cancellation | An expired or cancelled queued command cannot begin effects. A request that already started cannot be described as certainly unexecuted. | A disconnected client may not be detected immediately. Lost responses remain ambiguous. |
| Server lifecycle | Cancellation stops admission, closes accepted sockets, stops owned workers, and joins connection threads before returning. | No universal hard return bound for uninterruptible kernel I/O or escaped descendants. |
| Generated transport | Generated H.264 and AAC traverse a bounded delayed TCP proxy. Disconnect releases owned resources. A fresh run records and fully decodes both streams. | This is a local software-codec transport trial, not the agent's RTMP and RTSP media path or a real two-host network. |
| Recovery | Failed effects do not silently restart output. Explicit Stop retains unconfirmed cleanup obligations until release. | In-memory recovery is not durable replay or exactly-once remote execution. |

Inspect [the current verification record](verification.md) for executed results. The contracts above are requirements, not implied passes.

Ordinary Stop remains serialized behind active preparation. Per-instance server shutdown requests cooperative cancellation, but an already-blocked call may wait for its current deadline. Virtual integration of the actual agent RTMP and RTSP media path, decoded-frame freshness, and broader congestion recovery remains open. The separate generated TCP test does not close that software gap, and hardware testing is not needed to address it.

## Independent physical acceptance

These conditions remain separate from the virtual matrix.

- Direct Scanout behavior with the selected compositor, display mode, and workload.
- GPU, CPU, memory, thermal, and power contention under matched game-only and capture workloads.
- Actual capture-helper privileges, device permissions, and selected source provenance.
- PipeWire and physical audio behavior, microphone isolation, and local gameplay audio independence.
- Real GSR capture and production OBS decode, preview, recording, synchronization, frame age, and bounded media queues.
- Actual host network isolation, SSH policy, cgroup containment, and authorized forced-death cleanup.

Follow [the operational runbook](operations.md) only with separate authorization for those tests. Keep `rustCandidate=false` and empty autostart targets. A virtual pass does not change those gates.
