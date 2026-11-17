# S26 — merged-main Dreaming reconciliation

Date: 2026-11-17 UTC. This is a documentation-only inspection and knowledge correction. Its source scope is the public `tenzyu/splitcast` main commit `ed7fe60e8efd0dea454346b13f38a680096fb60e`; it is **not** a fresh runtime test receipt.

## Authority and observations

| Source / operation | Observation | Strict limit |
| --- | --- | --- |
| GitHub `GET /repos/tenzyu/splitcast/branches/main` and recursive tree | main points to `ed7fe60`; runtime tree includes `packages/splitcast-rs/src/poc_speech.rs` and existing memory/acceptance docs | Source existence does not show execution on an installed host |
| GitHub `GET /repos/tenzyu/splitcast/pulls?state=all`, followed by PR #9 detail | PR #4 through #9 merged; #9's merge commit is `ed7fe60`. PR #2/#3 were already merged | No deployment or new PoC acceptance follows |
| [GitHub Actions run 44097472963](https://github.com/tenzyu/splitcast/actions/runs/44097472963), jobs endpoint | At main `ed7fe60`, `verify`, `verify-virtual`, `Native and WASM packages`, `Delivery scripts`: **all completed with success** | Hosted matrix is not real Twitch, physical game, microphone, cross-host playback, or gameplay tail-latency evidence |
| [Prior failing workflow 44089362405](https://github.com/tenzyu/splitcast/actions/runs/44089362405) | Historically failed; #6 subsequently changed the over-strict asynchronous Stop regression test and was merged | No retroactive PASS or proof of hard shutdown bounds |
| [Manual cache run 44097570226](https://github.com/tenzyu/splitcast/actions/runs/44097570226) | `in_progress` at inspection | Neither cache release success nor failure established |
| [`poc_speech.rs`](../../packages/splitcast-rs/src/poc_speech.rs), [`main.rs`](../../packages/splitcast-rs/src/main.rs) | `poc-play` validates a bounded explicit local WAV and target; hands PCM to `pw-play`. Receipt is `local_fixture`, `native_player_process`, `listening=unverified` | No event ingestion, synthesis, return transport or physical listening |
| [`agent.rs`](../../packages/splitcast-rs/src/agent.rs), `Agent::prepare` | Refuses enabled `eventsub_enabled`, `tts_enabled` and `tts_mix_remote`; current speech delivery remains unfinished | Do not remove refusal or call the finite CLI integrated TTS |
| [S25 convergence checkpoint](poc-convergence-20261117.md), [handoff](../handoff/poc-20261117.md) | Selected Twitch channel and path to suitable existing OAuth absent in surveyed scope; approval for WSS/Helix/chat v1 already granted. GSR/helper, proper mic input and authorized worker launch remain unresolved | Dated environment inventory is not automatically current host state; missing token is not scope rejection |

## Corrections to preserve

1. **Merge state is newer than the handoff.** PR #9 no longer needs conflict resolution or merge. PR #6 is no longer an unmerged candidate. Earlier notes are preserved as dated checkpoints with a prominent supersession pointer.
2. **A test-only correction addresses an oracle assumption, not the entire failure domain.** A bounded HTTP `202 requested` permits later `completed`; a PASS for the amended test says nothing universal about parser admission, controller SIGKILL, lost remote ownership or kernel-stuck workers.
3. **Separate capability from proof.** Bounded local WAV playback, VOICEVOX synthesis, NVENC/NVDEC probe, generated production-only media verification and hosted CI are different evidence classes. None alone demonstrates real Twitch-triggered, production-to-capture acoustic output.
4. **Converge on the user goal.** The PoC requires selected game video, game-only audio and microphone offload, plus Twitch-triggered VOICEVOX return audio. Camera is excluded. Offloading OBS cannot remove capture-side encoding or its interference. Do not grow a generic control platform, duplicate receiver, GUI or custom transport without an evidenced missing behavior.
5. **Respect existing authority.** WSS and necessary Helix operations with existing suitable OAuth were approved, but account/channel inputs are still required. No new OAuth, chat posting, public broadcast, arbitrary capture-host SSH execution, capability installation, system switch or deployment is inferred. Capture-host read-only SSH cannot launch the finite player.
6. **Three independent acceptance surfaces remain.** Gameplay all-frame OFF/ON tail behavior; decoded-content media freshness/synchronization; and real event-to-physical-playback latency and failure isolation. Never infer frame freshness from byte progress, or cross-host one-way latency from unsynchronized clocks.

## Resume contract

Use the [PoC handoff](../handoff/poc-20261117.md) for the detailed limits and [S25](poc-convergence-20261117.md) for observed dependency gaps, with **this** page taking precedence for current Git merge/CI status.

**Next discriminating input:** selected Twitch channel plus path to an existing suitable OAuth configuration, or an existing receiver's supported local event output. Never ask for token contents or repeat already granted WSS/Helix permission. If unavailable, preserve the blocker and work only on independent, source-testable behavior. The subsequent bounded event-admission / synthesis test must distinguish fixture success from a real `channel.chat.message` v1 receipt.

After that, independently authorize finite capture-host execution and acquisition prerequisites; only then perform temporary integrated trials, independent decoding and fixed-workload game OFF/ON comparisons. Stop before unrestricted remote execution, permanent settings, autostart, public output or OS activation.

## Verification and provenance for this Dreaming pass

- Performed: GitHub ref, tree, PR metadata, hosted run/job status, and targeted current source/doc inspection; reviewed and corrected memory and risk links. This page records those operations without treating them as application tests.
- **NOT RUN:** `cargo run -p splitcast-xtask -- verify` locally; `verify-media`; physical device, SSH or Twitch commands; cache publication; operating-system activation. Hosted CI has its own evidence identity.
- Primary public evidence: [current main commit](https://github.com/tenzyu/splitcast/commit/ed7fe60e8efd0dea454346b13f38a680096fb60e), [CI run](https://github.com/tenzyu/splitcast/actions/runs/44097472963), [PR #9](https://github.com/tenzyu/splitcast/pull/9), [PR #6](https://github.com/tenzyu/splitcast/pull/6), and current source paths. Earlier private receipts remain private.
