# SplitCast memory

Updated 2026-11-13. Scope: SplitCast decisions, failures, and evidence across the source monorepo and this standalone repository. This is a knowledge index, not deployment authorization.

## User instruction

過去のセッション、コード、判断、検証結果を読み、将来のエージェントが同じ調査・失敗・設計判断を不必要に繰り返さずに済む知識を構築する。特に「何を実現しようとしたか」「なぜ採用・却下したか」「どの観測が仮説を覆したか」「まだ何が分からないか」を、複数のセッションとリポジトリを横断して結びつける。カテゴリーやページ境界は利用価値に従って変更してよい。根拠のない確定、異なる環境・日時・リポジトリの混同、原資料の破壊、未確認事項の検証済みへの昇格はしない。知識の更新は根拠への参照を残し、以前の判断を追跡できるようにする。

## Current boundaries

- The goal is standalone OSS SplitCast, outside the source monorepo, with configurable capture host and production host roles. Personal machine configuration is not a product requirement.
- Historical Rust publication was explicitly partial and disabled. Feature parity, real game capture, production OBS acceptance, and competitive non-regression were not established.
- Historical results belong to the source implementation documented at `21a4f88`, including repair `0fbad46`. Extraction baseline `3c8857353581424e5cca517d7ccf0db8de7514d4` is a different identity, not a new test result.
- Fresh standalone build, test, and runtime evidence is pending in this memory. Do not relabel imported results as standalone verification.
- No build, historical approval, or memory update authorizes host activation, privileges, external listeners, public streaming, or game changes.

## Read by question

| Question | Page |
| --- | --- |
| What was adopted or rejected, and why? | [Decisions](docs/memory/decisions.md) |
| What actually ran, against which artifact? | [Evidence and source register](docs/memory/evidence.md) |
| What remains unknown or unsafe to assume? | [Open risks](docs/memory/open-risks.md) |
| How do new findings correct old knowledge? | [Update protocol](docs/memory/update-protocol.md) |

Read [the Rust entrypoint](RUST_AGENT_ROOT.md) and [engineering policy](RUST_ENGINEERING_AGENTS.md) before nontrivial Rust work. Historical documents under `docs/history/source/` are evidence, not current operating instructions.
