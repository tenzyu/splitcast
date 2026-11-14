# SplitCast memory

Updated 2026-11-14, Japan Standard Time. Scope: SplitCast decisions, failures, and evidence across the source monorepo and this standalone repository. This is a knowledge index, not deployment authorization.

## User instruction

過去のセッション、コード、判断、検証結果を読み、将来のエージェントが同じ調査・失敗・設計判断を不必要に繰り返さずに済む知識を構築する。特に「何を実現しようとしたか」「なぜ採用・却下したか」「どの観測が仮説を覆したか」「まだ何が分からないか」を、複数のセッションとリポジトリを横断して結びつける。カテゴリーやページ境界は利用価値に従って変更してよい。根拠のない確定、異なる環境・日時・リポジトリの混同、原資料の破壊、未確認事項の検証済みへの昇格はしない。知識の更新は根拠への参照を残し、以前の判断を追跡できるようにする。

## Current boundaries

- The goal is standalone OSS SplitCast, outside the source monorepo, with configurable capture host and production host roles. Personal machine configuration is not a product requirement.
- Historical Rust publication was explicitly partial and disabled. Feature parity, real game capture, production OBS acceptance, and competitive non-regression were not established.
- Historical results belong to the source implementation documented at `21a4f88`, including repair `0fbad46`. Extraction baseline `3c8857353581424e5cca517d7ccf0db8de7514d4` is a different identity, not a new test result.
- Fresh standalone workspace checks, 110 ordinary tests, two isolated local components, Rust/WASM builds, module assertions, and consumer gate assertions now have separate evidence in E07–E10. They do not establish live deployment acceptance. Full consumer flake evaluation still fails in an unrelated Firefox derivation, also reproduced at the untouched extraction baseline.
- The later S16/E12 virtual-software run starts at `aa86fe0` plus task changes. Its final extended verifier passes 124 ordinary tests and three separate local components. Two actual local agents exercise control and recovery with stand-in workers. Queue cancellation, unknown outcomes, connection ownership, and shutdown have new focused evidence. Ordinary Stop preemption, actual agent media integration, durable outcomes, browser ownership, and physical acceptance remain open. See [virtual system acceptance](docs/virtual-acceptance.md).
- S18 preserved prior source at `7585308` and closes P0 for `d2205d0` on pinned `x86_64-linux`: hosted CI, signed GitHub Releases publication and a fresh signature-enforced no-build consumer all pass in run `43561723796`. The operator approved the three custom outputs and corresponding sources after GPL/FDK-AAC disclosure, plus merge and a verification prerelease, not deployment. Failures remain recorded, including unpublished draft `445034829`. Start with [current state](docs/memory/session-state.md), [download instructions](docs/binary-cache.md), and [the CI/cache evidence](docs/evidence/ci-nix-cache-20261114.md). Next is P1's actual agent media path; P2–P5 remain open.
- No build, historical approval, or memory update authorizes host activation, privileges, external listeners, public streaming, or game changes.

## Read by question

| Question | Page |
| --- | --- |
| What was adopted or rejected, and why? | [Decisions](docs/memory/decisions.md) |
| What actually ran, against which artifact? | [Evidence and source register](docs/memory/evidence.md) |
| What remains unknown or unsafe to assume? | [Open risks](docs/memory/open-risks.md) |
| How do new findings correct old knowledge? | [Update protocol](docs/memory/update-protocol.md) |
| What should the next session do without repeating this work? | [Current working state](docs/memory/session-state.md) and [CI/cache handoff](docs/handoff/ci-nix-cache.md) |

Read [the Rust entrypoint](RUST_AGENT_ROOT.md) and [engineering policy](RUST_ENGINEERING_AGENTS.md) before nontrivial Rust work. Historical documents under `docs/history/source/` are evidence, not current operating instructions.
