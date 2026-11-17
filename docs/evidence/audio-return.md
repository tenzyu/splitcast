# Prepared Agent speech return

On the development host, a production Agent accepted a finite utterance, called the loopback VOICEVOX query and synthesis endpoints, and handed the resulting WAV to its capture owner over an authenticated SSH forward. The capture Agent validated the reply and launched the native PipeWire player. The test compared the entire PCM input and independently located the signal at the selected virtual sink. The two processes and all audio nodes ran on one Linux kernel.

## Protocol and ownership

Set `tts.enabled = true` and the same `tts.expiry_seconds` on both roles, choose an explicit `audio.tts_sink` node name on capture, and leave EventSub and remote mixing off. The normal capture-side Prepare establishes the SSH forwards, validates the route contract and obtains a production ownership lease. Speech works in Ready and Running. Recording or publication is unnecessary. An older or differently configured peer fails the enabled contract comparison.

In a separately authorized prepared session, use the existing production CLI to get an ID and submit one intentional utterance.

```sh
splitcast --config production.toml tts status
splitcast --config production.toml say 'Speech text' --id "$utterance_id"
```

Set `utterance_id` to the returned `next_id`. It belongs to this preparation and expires on the production clock. Concurrent clients can observe the same ID. The first successful admission consumes it. Only use a fresh ID for a new intentional utterance. After an uncertain result, retain the original ID. Reissuing the same text under a new ID requests another utterance.

One synthesis worker serves the configured queue (1–32 jobs). Each WAV is at most 4 MiB, 30 seconds, one or two PCM16 channels at 8–48 kHz. The Agent keeps queued audio in memory. An owner-authenticated `tts-next` removes one item before encoding its reply. Its maximum encoded body is 5,592,408 bytes plus 512 bytes of envelope. A lost reply can lose that utterance. Capture accepts a strictly increasing ID from its prepared epoch and runs at most one player. Only capture configuration selects the target.

The deadline covers admission, queueing, synthesis, transfer and playback. Capture anchors the remaining grant at its own monotonic poll start, so transfer time consumes part of the budget. Stop, shutdown, lease failure and transport failure use the ordinary session cleanup path. Player errors release the group and surface in capture warnings. An unconfirmed player cleanup keeps the session from reporting a completed release. Process-group escape, a killed controller and uninterruptible kernel I/O remain limits of the enclosing service.

## Software observations

The normal fault suite uses two actual Agent processes, distinct bearer credentials, real loopback HTTP and an instrumented player. The native test additionally uses the Agent's OpenSSH `-L` forwards, a disposable pinned host key and client key, and local `sshd -i` over a stdio proxy. It starts a private PipeWire server and WirePlumber linking policy with generated null sinks. Host hardware discovery and remote shell sessions are outside the fixture. The VOICEVOX HTTP service returns controlled WAVs. Unrelated OBS and media workers are stand-ins.

| Observation | Independent check |
| --- | --- |
| Initial native return | A tap opens the observed PCM as `pw-play` stdin. All 192,000 bytes matched SHA-256 `02311f69c7b13d9318f96e11827ed533f4594d3d950f3e2ebf0f61facd5c3a9c`. A 4,096-byte interior marker appeared once at `return.selected` and was absent at `game.local`. |
| Lost SSH forward during an active return | Kill the owned SSH worker, observe the forward refusing connections and the player PID reaped. Play a separately generated marker on the independent game sink during the outage and find it in that sink's monitor. |
| Missing target and disappearing receiver | Prepare with an absent sink and submit distinct PCM. Its marker was absent from both monitored destinations. Remove a different sink while it is rendering, after observing its fixture marker. The player exited and sampled blocks from that signal were absent from the game sink. |
| Explicit recovery | Stop, restore the target and Prepare again. A different 192,000-byte utterance matched SHA-256 `93f1473278ca8a579417c777994efe30028c524a015fa8911c221dabe6947b64` at player stdin. Its marker appeared once on the selected sink. |
| Rejection, pressure and cancellation | Reject incorrect bearer/owner, an injected target, malformed WAVs, expired IDs and stale session IDs. Concurrent use of one ID admits one job. A full queue rejects new work. Stop and expiry reap a stalling player. A fresh session delivers different PCM after failure. |
| Uncertain outcomes | Discard a real submission reply and retry its ID. The player-start ledger records one attempt. Under a separate owner, discard a real pull reply, independently observe that the ready item was consumed and confirm the next authenticated pull is empty. |

## Verification record

Base `7d63a7d1ef985a0a50797c856b88cdf8699ed663` passed [all four main CI jobs](https://github.com/tenzyu/splitcast/actions/runs/44197918761). Runtime and tests for the expanded evidence are at `5cfa54863276888cba0a7f443e41355a6c3ed16e`. The operator's intermediate commit `37e4cc8` is preserved. The repository-local verifier retains individual commands, logs, revision and working-tree records under `artifacts/verification/runs/`.

| Command | Observed result |
| --- | --- |
| `git ls-remote git@github.com:tenzyu/splitcast.wiki.git HEAD` | Exit 128, repository not found. The versioned Wiki entry point for these changes is `docs/memory/index.md`. |
| `nix develop -c cargo check --workspace --all-targets --locked --offline` | Exit 0 on the initial integration. |
| `nix develop -c cargo test -p splitcast-rs-core --test two_agents return_tests::return_faults_and_replay --locked --offline -- --exact --nocapture` | First run exited 101. The lost-reply test client omitted the required Origin header and its synthesis fixture timed out. With the authenticated request fixed, the test passed and observed five player starts. |
| `nix develop -c cargo test -p splitcast-rs-core --test two_agents return_tests::native_return --locked --offline -- --exact --ignored --nocapture` | First run exited 101 during Prepare. OpenSSH rejected the disposable authorized key under a shared temporary-directory ancestor. The fixture already owned its private directory. Configuring its stdio sshd to use those exact key files without checking shared ancestors resolved it. The next run passed. An additional run including live sink removal passed. |
| `nix develop -c cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | Exit 0. |
| `nix develop -c cargo run -p splitcast-xtask -- verify` | Exit 0, all 11 checks passed in run `1794958291182233345`. Its dirty source on `37e4cc8` was committed as `5cfa548`. The earlier no-default-features connection reset recorded on `9421651` remains a failure of that historical run. |
| `nix develop -c cargo run -p splitcast-xtask -- verify-virtual` | Exit 0, all 15 checks passed. Run `1794958428555444891` recorded clean source `5cfa548`, Rust 1.95.0, Linux x86_64 and Cargo.lock Git blob `0fcbbdafa7b551161d3ff595e6f2c4ec878a4fc6`. The ordinary suite ran 168 tests per feature selection. The virtual additions exercised private audio, generated media, native return and local transport. |
| `python3 tools/ci/check_docs.py` | Exit 0, 47 Markdown files, zero broken local links and zero OKF issues. |

The intermediate [PR CI run](https://github.com/tenzyu/splitcast/actions/runs/44199994508) failed its format checks on `37e4cc8`. Its two verifier artifacts show the ordinary and native return tests passed on that source. The local clean-source result above includes the formatting correction.

## Rollback and remaining gates

Use the ordinary capture Stop and wait for owner-scoped completed/Idle and release of the player and forward. For a disabled configuration, set `tts.enabled = false` on both stopped roles. A source rollback can use the pinned base in a separate checkout, keeping the current branch and its evidence. The Nix runtime gate remains false and services have no autostart targets.

Software acceptance ends at observed native-player input and private sink monitors. Physical listening, real VOICEVOX speech quality, actual host routes and network behavior, competitive game input/audio/frame tails, and event-to-acoustic latency need separate trials. No cross-host elapsed time was calculated. OAuth, physical capture, streaming and deployment were outside this run.
