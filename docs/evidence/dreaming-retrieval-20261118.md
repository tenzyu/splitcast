# S33 · Source and retrieval audit, 2026-11-18 JST

## Source snapshot

Inspected Git objects on the development host and read GitHub PR/Actions metadata. No application run or host observation originated in this pass. The [revision guide](../memory/proposed-revisions.md) links the code and specific experiments.

| Source | Commit | Git tree | Disposition at inspection |
| --- | --- | --- | --- |
| main | `e92b5db390835041b33de46618c49d5f98cad9cf` | `e65993208122c954e5b27a66502d877f5179fcef` | [#17 merged](https://github.com/tenzyu/splitcast/pull/17) after [#16](https://github.com/tenzyu/splitcast/pull/16). |
| #18 | `5485ca89ca44259f10c7c2e1282841c35a7dce0f` | `43af49d7922dc9bada2ea512fefceb70c0f55edc` | Draft on main. Hosted CI success. The generated-media receipt was taken at implementation `dc730aa2`. |
| #19 | `51bd829d56169b930d60315d2682f0c4c6eec2f9` | `04d2dc24567fbb868fff82246c7e91b54094f77e` | Draft on main. Hosted CI success. Earlier hosted run `44254418762` at `8557945` failed. |
| #20 | `6fcec4a3a6882937bb8a7b6740b15e2affd0ea30` | `9f6b1045690d4a01b815321f5724fa6b0b5df870` | Draft on #19. Merge base with #19 equals its exact head. Hosted CI success. Native GUI compared `04f60e32` and `aab9907d` with fixture source `fb847b8b`. |
| Local detached merge | `0b27a9fc340407431ad5f1288a652fde77650f04` | `e2eae1cf3cfb649cfbfe4df90091c1cbbe897444` | First-parent merges `c7b91160` (#18), `6a655e4b` (#19), then `0b27a9fc` (#20). Local object availability can differ in a fresh clone. |

The detached checkout still contained two independently inspectable summaries. The verifier run `1794993182719940596` identified the detached commit, an empty `working-tree.txt` and 14 PASS/0 rows in `summary.tsv` (SHA-256 `90782c4ab6375f7402a8c8935d6629de1e8d7e04924224481e04b34da22172f3`). Media run `20261118T092610Z-Q2VwAzzg` had identical source-before/after files naming that commit, sandbox exit 0 and seven cases, complete matrix, unchanged source, PASS in `summary.json` (SHA-256 `92da821f55abaa60e014a34a458f5795411fb1abb1b122f84d4b7776e1687d0d`). The summaries remain local. These identities let a reader compare a supplied artifact. Absent the files, cite this inspection as a report. The combined tree has no independent hardware or deployment acceptance.

Recheck on the development checkout with the PR queries in the [update protocol](../memory/update-protocol.md#when-a-proposal-moves), then inspect the Git objects:

```sh
git show -s --format='%H %T %P %s' e92b5db 5485ca89 51bd829 6fcec4a3
git merge-base 51bd829 6fcec4a3
git log --first-parent -3 --format='%H %P %s' 0b27a9fc
gh run view "$run" --json headSha,event,conclusion
```

The run query returned `pull_request` and `success` for [44254307055](https://github.com/tenzyu/splitcast/actions/runs/44254307055), [44268693685](https://github.com/tenzyu/splitcast/actions/runs/44268693685) and [44272711325](https://github.com/tenzyu/splitcast/actions/runs/44272711325), and `failure` for [44254418762](https://github.com/tenzyu/splitcast/actions/runs/44254418762). The recorded event head and tested checkout may differ. The #19 failure receipt identifies the actual checkout and matches its tree to the former head.

## Why change the routes?

On clean main, `python3 tools/ci/check_docs.py` exited 0 with `Markdown=49 local_links=260 broken=0 okf_issues=0`. The clean pinned wiki scan counted 7 stale candidates, 3 historical and 3 unaffected pages. The Markdown checker checks local files and ignores fragments. The open-risks link to the former `current-correction-ledger-2026-11-17` heading therefore passed. The Dreaming page also kept #9's live speech refusal in present tense even though #16's dated receipt identified an operator-heard event. The legacy uppercase index already forwarded to the canonical page.

Kept the single startup capsule, explicit search and one comparison page. The [Anthropic study](https://www.anthropic.com/research/components-of-agent-memory) found value in selective notes and source search, including observations of success. Its results depend on its corpus. For this pass, one writer reconciled source evidence, preserving the earlier failure alongside each positive control. [OpenAI’s model guide](https://developers.openai.com/api/docs/guides/latest-model) informed the bounded assignment and final check. T3 reported provider `codex_2` (Codex-2), model `gpt-6.2-astra`, reasoning effort `xhigh`, consistent with the running harness.
