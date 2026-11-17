# SplitCast memory

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

PR #3 was merged into main (merge `a5e7a22`, 2026-11-17). S20/E16's generated production-only pass at
`42a64e4` retains its scope and AV01 correction; S22 separately records the new
review fixes and gates. P1 is narrowed, not closed. Neither source publication,
test success nor historical permission authorizes further merge, deployment, capture,
private-host access, privilege changes, external listeners or public output.

## Read by question

| Question | Page |
| --- | --- |
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
