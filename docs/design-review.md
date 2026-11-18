# SplitCast semantic review — surviving contracts

The historical findings below retain their original scope. The PR #16
follow-up records the current contract and unresolved acceptance gates.
[Original 2026-11-13/14 review](https://github.com/tenzyu/splitcast/blob/archive/pre-dreaming-20261117/docs/design-review.md) · [Decision and evidence index](memory/index.md)

## Findings and disposition

| ID | Initial observation / mechanism | Corrected or still open |
| --- | --- | --- |
| **F01 P1** | NixOS module exported by prematurely applying a function requiring `config/lib/pkgs` | Fixed in extraction; actual consumer later exposed duplicated Home Manager options through two imports. Canonical version-specific HM module key/path fixed targeted consumer gates. Not a deployment test. |
| **F02 P1** | Detached connection threads, untracked joins, serialized Prepare/Stop and incomplete bounded shutdown | S16 introduced listener owner, tracked joins and per-operation cancellation; merged PR #2 adds listener-side urgent Stop admission. Per-worker cleanup, parser-slot exhaustion, kernel-stuck tasks and controller SIGKILL remain limits. |
| **F03 P1** | Timed-out/dropped RPC receiver can leave queued Start/settings effect executing later | Queue expiry/cancellation before effect and explicit *unknown outcome* after effects added in S16; no durable transaction log, exactly-once semantics or safe blind Start retry. |
| **F04 P2** | `Config` public fields can bypass parser and invalidate sink/filter/role invariants | PR #16 closes field mutation across the crate boundary. Runtime consumers inside the crate and media provenance still require review. |
| **F05 P2** | libc/FFI sites lack complete adjacent safety and ABI proof; process identity depends on unreaped leader | No demonstrated UB or custom frame ABI; no Miri coverage of real Linux subprocess/signal lifecycle. Adjacent proofs and Linux target contract still needed. |
| **F06 P2** | String errors collapse protocol categories; cleanup error may mask initiating failure; post-rename fsync can report failure after visible commit | PR #16 distinguishes pre-effect rejection, unknown command outcome and incomplete Stop. Durable commit and replay history remain open. |
| **F07 P2** | Browser fetch futures lack ordering ownership; older response may overwrite newer status; token snapshots persist | The native Web client serves fixed assets from the control listener. The bearer stays in the page, normal controls serialize, and a newer Stop supersedes late display updates. CLI and TUI use the same authenticated command parser. |

## PR #16 review follow-up

The mandatory review was submitted against `0823165`. The disposition below
describes source changes and outstanding evidence. [Review](https://github.com/tenzyu/splitcast/pull/16#pullrequestreview-5950165310) · [Verification](verification.md)

| Finding | Disposition |
| --- | --- |
| 1. Competing output deadlines | One production watchdog replaces the trial timers. Public Start requires confirmation, a duration and the prepared session's current Start ID. Expiry fences activation and admits Stop. Capture retains the renewable owner lease. |
| 2. Late renders and OBS SIGSEGV | OBS is owned directly. Output inactivity, normal exit and forced cleanup have separate observations. A generated-input comparison reproduced the recorded null dereference after SIGTERM. SIGINT produced a normal exit in that comparison and in the integrated physical trials. Both controlled 60 fps software render measurements exceed 1%. Hardware OBS startup and Xorg rollback remain blocked. [OBS diagnostic](evidence/pr16-obs.md) |
| 3. EventSub exhaustion | The prepared-session worker reconnects on transient failures, preserves subscription identity, and bounds recent chat IDs by age and count. Revocation and invalid identity end the subscription. |
| 4. Manual speech and control | Clients obtain the current speech ID and submit once. Stop observation correlates the current admission with its completion. Remote completion remains owner-bound. |
| 5. Missing combined physical evidence | Source-bound trials verified the two-host game, camera and SoloCast/RNNoise path, a 302.036 s private Twitch bandwidth test, and independent public archive playback. All 120 matched replay observations preserved Direct Scanout and the selected audio links. Gameplay frame times, camera age, physical A/V alignment and speech playback during the public game window remain open. [Physical and viewer evidence](evidence/pr16-managed-acceptance.md) |
| 6. Encapsulation and source consistency | The public Config boundary requires validated construction and exposes read-only access. PID identity, the generation fence, and the remote release proof remain part of the contract. Both role examples and the operator runbook describe the revised commands. |

## Structural invariants

- `lifecycle.rs` phase encodes authority and output, `OwnerId` validates identity, and `Workers` owns typed process handles. Cleanup evidence comes from the owning process and peer.
- Process groups require unreaped leaders for PID-group identity; a child can `setsid` escape, a killed controller cannot perform Drop, and uninterruptible kernel I/O is not bounded by user deadlines. The service cgroup is required containment.
- A remote Prepare whose response is lost may have committed. Record the remote cleanup obligation at the positive socket write and retain it until owner-specific completion. Observe completion separately from `202 requested` admission.
- Shared token-bearing strings may be cloned per in-flight connection; token replacement/revocation of stored state does not erase those copies. Do not promise zeroization.
- `state.rs` temp-write → fsync → rename → directory fsync means post-rename failure can leave a visible runtime file despite error. Read-back or indeterminate-commit handling needed.
- Production EventSub binds chat to the configured user and subscription and retains recent message IDs within the session. Speech remains consume-on-read.
- Production records which OBS output may have started before sending its RPC. Cleanup reconnects to observe and stop that output before releasing the owned OBS process and profile. Public Twitch requires production opt-in, explicit confirmation and an operator-selected duration.
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
