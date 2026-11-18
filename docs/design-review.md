# SplitCast semantic review — surviving contracts

**Current use:** scope-limited review history; not a current source-line oracle or permission to deploy. The [full 2026-11-13/14 review](https://github.com/tenzyu/splitcast/blob/archive/pre-dreaming-20261117/docs/design-review.md) is preserved at the pre-Dreaming archive ref. Source code, targeted tests and later [decision/evidence IDs](memory/index.md) override stale review line numbers.

## Findings and disposition

| ID | Initial observation / mechanism | Corrected or still open |
| --- | --- | --- |
| **F01 P1** | NixOS module exported by prematurely applying a function requiring `config/lib/pkgs` | Fixed in extraction; actual consumer later exposed duplicated Home Manager options through two imports. Canonical version-specific HM module key/path fixed targeted consumer gates. Not a deployment test. |
| **F02 P1** | Detached connection threads, untracked joins, serialized Prepare/Stop and incomplete bounded shutdown | S16 introduced listener owner, tracked joins and per-operation cancellation; merged PR #2 adds listener-side urgent Stop admission. Per-worker cleanup, parser-slot exhaustion, kernel-stuck tasks and controller SIGKILL remain limits. |
| **F03 P1** | Timed-out/dropped RPC receiver can leave queued Start/settings effect executing later | Queue expiry/cancellation before effect and explicit *unknown outcome* after effects added in S16; no durable transaction log, exactly-once semantics or safe blind Start retry. |
| **F04 P2** | `Config` public fields can bypass parser and invalidate sink/filter/role invariants | Validation remains constructor-time, not a universal invariant-bearing public API. Restrict fields/validated updates before stable library claims. |
| **F05 P2** | libc/FFI sites lack complete adjacent safety and ABI proof; process identity depends on unreaped leader | No demonstrated UB or custom frame ABI; no Miri coverage of real Linux subprocess/signal lifecycle. Adjacent proofs and Linux target contract still needed. |
| **F06 P2** | String errors collapse protocol categories; cleanup error may mask initiating failure; post-rename fsync can report failure after visible commit | Preserve cause, operation/cleanup status and unknown-commit semantics in stable typed response; no assumption that timeout means no effect. |
| **F07 P2** | Browser fetch futures lack ordering ownership; older response may overwrite newer status; token snapshots persist | The native Web client serves fixed assets from the control listener. The bearer stays in the page, normal controls serialize, and a newer Stop supersedes late display updates. CLI and TUI use the same authenticated command parser. |

## Structural invariants, not overclaims

- `lifecycle.rs` phase encodes authority/output; `OwnerId` validates creation; `Workers` owns typed process handles. Those compile-time invariants do **not** prove network/worker cleanup or resource isolation.
- Process groups require unreaped leaders for PID-group identity; a child can `setsid` escape, a killed controller cannot perform Drop, and uninterruptible kernel I/O is not bounded by user deadlines. The service cgroup is required containment.
- Lost remote Prepare response may still have committed: record remote cleanup obligation before positive socket write, retain it until owner-specific completion. A `202 requested` Stop is admission, **not** final release.
- Shared token-bearing strings may be cloned per in-flight connection; token replacement/revocation of stored state does not erase those copies. Do not promise zeroization.
- `state.rs` temp-write → fsync → rename → directory fsync means post-rename failure can leave a visible runtime file despite error. Read-back or indeterminate-commit handling needed.
- `tts.rs` synthesis worker's in-memory dedup/cleanup is not persistent exactly-once audio delivery. The production-only EventSub worker binds chat to the configured user and subscription. The session holds a bounded replay set and speech remains consume-on-read.
- Production records which OBS output may have started before sending its RPC. Cleanup reconnects to observe and stop that output before releasing the owned OBS process and profile. Public Twitch requires both production configuration and an explicit Start selection.
- `publish=false` crates, Rust 1.95 x86_64-linux flake, the internal command protocol is versioned for a matching native pair.

## Performance/acceptance

Game input/audio/update/draw/display is upstream of production. Native GSR, FFmpeg, MediaMTX, OBS and SSH favor reuse/process isolation but still consume CPU/GPU/memory/power; TCP head-of-line blocking can accumulate stale media. Do not invent a custom transport, zero-copy hot path or clone optimization without an observed bottleneck. The evidence separates gameplay OFF/ON/OFF tail behavior, independent decoded-content media freshness/sync, and real event-to-return playback latency. Generated production-only media and tests cannot prove hardware/gameplay non-regression.

The current candidate has one native protocol and fixed routes. Agent startup
owns the state lock and control listener. Prepare owns SSH, the lease and
speech processing, plus MediaMTX for a media session. Start owns native
capture, private Pulse/RNNoise and OBS; there is one optional camera and one
explicit output per session. The request deadline and Stop path remain
separate from observation because lost replies can hide committed effects.
The Web assets and terminal menu share the native protocol, and voice
delivery keeps its bounded epoch and selected-player ownership.

**Preserve:** `rustCandidate=false`, no autostart, explicit output and selected game-only audio, no game hook/Vulkan-Zink/silent portal fallback. Do not activate hosts or public listeners by documentation or build result. For exact historical reviews, failures and test commands, retrieve [original source](https://github.com/tenzyu/splitcast/blob/archive/pre-dreaming-20261117/docs/design-review.md); for newer statuses use [risks](memory/open-risks.md), [verification](verification.md), [virtual design](virtual-control-design.md), and [PoC handoff](handoff/poc-20261117.md).
