# SplitCast memory

## 2026-11-17 main reconciliation (S26 / E21)

**Authority:** GitHub's `main` ref `ed7fe60e8efd0dea454346b13f38a680096fb60e`, repository tree and the four hosted jobs in [CI run 44097472963](https://github.com/tenzyu/splitcast/actions/runs/44097472963). This is later than the paused S25/PR #9 handoff. Read [the S26 Dreaming receipt](docs/evidence/dreaming-main-20261117.md) for narrow claims and limits.

- [PR #4](https://github.com/tenzyu/splitcast/pull/4), [#5](https://github.com/tenzyu/splitcast/pull/5), [#6](https://github.com/tenzyu/splitcast/pull/6), [#7](https://github.com/tenzyu/splitcast/pull/7), [#8](https://github.com/tenzyu/splitcast/pull/8) and [#9](https://github.com/tenzyu/splitcast/pull/9) **are merged**. PR #9's merge commit is the current main HEAD. The earlier statements that #6 was open or #9 needed base-conflict resolution remain dated evidence, not live action items.
- Current HEAD's hosted CI passed `verify`, `verify-virtual`, `Native and WASM packages`, and `Delivery scripts`. The earlier [failed run 44089362405](https://github.com/tenzyu/splitcast/actions/runs/44089362405) remains failed historical evidence; the test-only #6 fix does not prove universal Stop cleanup.
- `splitcast poc-play` exists in `packages/splitcast-rs/src/poc_speech.rs` and dispatches through `main.rs`. It validates an explicit fixture and launches one bounded local native player. Its JSON receipt explicitly says `provenance=local_fixture`, `completion_scope=native_player_process`, `listening=unverified`. It is **not** Twitch admission, cross-host transport or physical listening.
- Current `agent.rs::prepare` continues to refuse enabled EventSub, TTS and remote mixing. The PoC is incomplete: account/channel inputs and event admission, authorized capture-host playback execution, GSR/helper and microphone input/worker route, and matched gameplay/media/return-path acceptance remain separate gaps. Existing WSS/subscription/Helix approval is not a missing permission and must not be requested again.
- The manually triggered [release-cache run 44097570226](https://github.com/tenzyu/splitcast/actions/runs/44097570226) was **in progress at inspection**. Do not interpret it as a successful or failed publication. Existing signed-cache receipts retain their separate source identities.
- This is **documentation-only** reconciliation. No application command, local verifier, Twitch API request, SSH worker, device capture, recording, branch merge, production deployment or release publication was performed by S26. Previous operational permissions remain scoped and do not follow from CI success.

The canonical next work remains [the PoC handoff](docs/handoff/poc-20261117.md), as corrected by this section and S26. Historical environment-gap discovery, CI publication and merged-PR work are **not** the current starting task.

Updated 2026-11-17. This is a short pull-based knowledge index, not deployment authorization. Earlier checkpoints remain in the dated decisions, evidence register and Git history (`8486e81` preserves the previous index).

## User instruction

過去のセッション、コード、判断、検証結果を読み、将来のエージェントが同じ調査・失敗・設計判断を不必要に繰り返さずに済む知識を構築する。特に「何を実現しようとしたか」「なぜ採用・却下したか」「どの観測が仮説を覆したか」「まだ何が分からないか」を、複数のセッションとリポジトリを横断して結びつける。カテゴリーやページ境界は利用価値に従って変更してよい。根拠のない確定、異なる環境・日時・リポジトリの混同、原資料の破壊、未確認事項の検証済みへの昇格はしない。知識の更新は根拠への参照を残し、以前の判断を追跡できるようにする。

## Current mission and boundary

Protect capture-host osu! gameplay and local audio; offload production work and
independently return production-computed speech/audio for local listening.
SplitCast should stay a thin integration layer. Ultra Low Latency / Ultra
Lightweight are goals, not measured results or adopted numeric targets.
The current Agent refuses incomplete speech/EventSub. See D15, not an old
recording-first recommendation, for the renewed product goal.

PR #3 is merged at `a5e7a22` on 2026-11-17. S23's private readiness investigation
confirmed source inclusion, not deployment. The operator supplied the next PoC
implementation instruction in S24. Work is paused with a [PoC handoff](docs/handoff/poc-20261117.md)
and [S25's convergence checkpoint](docs/evidence/poc-convergence-20261117.md).
The operator authorized checkpoint publication and PR #9, then requested that
the branch incorporate current main to resolve conflicts. PR merge and deployment
are not authorized by that conflict-resolution request.
Fixture work is locally committed in `3c2f913`. Twitch WSS, selected-channel
subscription and existing suitable OAuth use are now approved, but the selected
channel and OAuth configuration are absent in the inspected scope. Capture SSH
remains read-only. Real Twitch-triggered returned audio, game A/V and microphone offload
remain required; camera is excluded. Baseline comes before proposed thresholds.

The pre-merge checkpoint recorded PR #3 as under review. S20/E16's generated production-only pass at
`42a64e4` retains its scope and AV01 correction; S22 separately records the new
review fixes and gates. P1 is narrowed, not closed. Neither source publication,
test success nor historical permission authorizes further merge, deployment, capture,
private-host access, privilege changes, external listeners or public output.

## Read by question

| Question | Page |
| --- | --- |
| Where should the next session resume? | **[PoC handoff](docs/handoff/poc-20261117.md)** |
| What can the authorized initial PoC prove now, and what blocks it? | **[Convergence checkpoint](docs/evidence/poc-convergence-20261117.md)** |
| How should agents consolidate and correct knowledge? | [Dreaming protocol](docs/memory/dreaming.md) |
| Where does the next environment-gap session start? | **[Environment-gap entry](docs/memory/environment-gap.md)** |
| Why SplitCast; what moves, what cannot move? | [PoC mission and boundaries](docs/memory/poc-mission.md) |
| What was adopted or rejected, and why? | [Decisions](docs/memory/decisions.md) |
| What actually ran, against which artifact? | [Evidence and source register](docs/memory/evidence.md) |
| What remains unknown or unsafe to assume? | [Open risks](docs/memory/open-risks.md) |
| How do new findings correct old knowledge? | [Update protocol](docs/memory/update-protocol.md) |
| What changed on PR #3; what remains? | [Current state](docs/memory/session-state.md) and [review follow-up](docs/evidence/pr3-review-20261117.md) |

**Later 2026-11-17 GitHub state:** [README PR #5](https://github.com/tenzyu/splitcast/pull/5) merged at `9ef4431` and [Dreaming PR #7](https://github.com/tenzyu/splitcast/pull/7) merged at `1678a0c`. The [failed PR #5 run 44089362405](https://github.com/tenzyu/splitcast/actions/runs/44089362405) remains failed historical evidence; merge did not make it PASS. [Test-only PR #6](https://github.com/tenzyu/splitcast/pull/6) remains open at this check. Its `verify` and `verify-virtual` jobs passed, while the native/WASM job was still running; check final CI before claiming all checks green.

Read [the Rust entrypoint](RUST_AGENT_ROOT.md) and [engineering policy](RUST_ENGINEERING_AGENTS.md) before nontrivial Rust work. Historical documents under `docs/history/source/` are evidence, not current operating instructions.
