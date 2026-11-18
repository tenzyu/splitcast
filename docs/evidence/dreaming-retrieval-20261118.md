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

## Evidence replay

Downloaded hosted `verify` artifact **14405676152** for the failed #19 job via `gh api repos/tenzyu/splitcast/actions/artifacts/14405676152/zip`. Its single `tests.log` (SHA-256 `a91d148e95bcecd2ae459eb644d26fd1bb630c2fdeff1b88f1f42de9e884ebe8`) contains the failed SIGTERM test, 409 and unknown outcome. The zip contains zero `mediamtx.log` entries. A search of the Actions console with `gh run view 44254418762 --job 134632547127 --log-failed` and a 409/deadline text filter had returned no matches. The uploaded test log settles the symptom. Only the local reproduction identifies the port collision.

Inspected the existing native GUI `before/focus.json` and `after/focus.json` on the development host. Both identify injected cursor 129. Before reports failure, 0 chat and 0 preview requests. After reports success, 1 chat and 2 preview requests. Their SHA-256 values are `2caa5c044b20644d8ee95de6d372c1cc8c7f3f467ca7c392e748efa30506a5db` and `c539f7bf3f60473cb44835a526b53b5eb87f4cf7f11ec6382ff6ddf853d8919b`. For public reproduction use the [packaged native comparison](https://github.com/tenzyu/splitcast/blob/6fcec4a3a6882937bb8a7b6740b15e2affd0ea30/docs/verification.md#L232-L272).

All three pinned proposals inherit the stale refusal at `docs/memory/dreaming.md:38` and the missing fragment at `docs/memory/open-risks.md:205`. The repair is on this documentation branch. At #20, `docs/verification.md:194` describes the earlier Xvfb checkpoint. The native result starts at line 202 of the same revision.

## Independent retrieval trial

Two fresh T3 research sessions received the same five questions, four source pins and eight-shell-invocation budget. Neither received this conversation or the rubric. Both had the required entrypoint on main. The second reader also had the wiki at `4abae0f0892a144210314fb2b121789e431a7407`. Both sessions resolved to Codex-2 / Astra / XHigh. The separate executed-model field was empty. Search-only had one recovered provider-capacity event. The grader applied the [preselected questions and criteria](retrieval-questions.md) to the final answers.

| Question | Search only, factual | Wiki + search, factual | Primary citation S / W | Decisive observation |
| --- | ---: | ---: | ---: | --- |
| Q03 hosted 409 | 1 | 1 | 1 / 1 | Both kept the original cause unknown and located red, reproduction and repair. |
| Q04 focus | 0 | 0 | 1 / 1 | Both explained native focus, visible versus hidden polling, the before/after requests and the occlusion limit. Neither described a rate limit or the cursor backoff. A finite sample does not establish that bound. |
| Q05 Stop | 0 | 0 | 1 / 1 | Both required the matching ticket and abstained after supersession. Neither stated the fence for already queued activation. S also omitted separate OBS inactivity/exit and cgroup observation. W included them. |
| Q07 OBS | 1 | 1 | 1 / 1 | Both reported the two denominators, software environment and open hardware target. |
| Q08 Xorg | 0 | 1 | 1 / 1 | Both retained the D-state timeouts, failed reboot, operator reset and safe off-host alternative. W also distinguished successful Weston rendering from OBS EGL failure. |
| **Totals** | **2/5** | **3/5** | **5/5 each** | One additional complete answer under the all-distinctions rule. |

The activity records show 8 versus 7 shell invocations and **31 Git/GitHub source commands each**, including startup and repeated reads. A multi-blob `git show` counts as one command. Neither reader repeated a failed approach or promoted a PR/fixture into main or hardware acceptance. One safety-relevant omission in S was the Q05 cleanup observation. Neither recommended new host effects. S reported 33,425 shell-output tokens before truncation. Comparable model input/output token usage was unavailable. The draft review retains the reader answers and replay commands.

This is one paired batch, hand-scored by the documentation author. It measures five questions on the first documentation commit. The other nine have deterministic routes only. General model-level uplift and time savings are **UNMEASURED**. After scoring, the Stop index label named the generation fence and the focus entry added preview cadence and cursor backoff. Those edits have no fresh model score.

## Documentation verification

The development-host work unit used the existing convergence contract `dreaming-retrieval-20261118`, SHA-256 `c4f0ed0e046e73c456fa95bd79b5586fe60c918d2012735d95c9fb453655a715`. Reviewed `.envrc` and ran `direnv allow` (exit 0). The shell used explicit Nix wrappers for project checks.

| Command at the first docs commit `4abae0f` | Result |
| --- | --- |
| `nix develop -c python3 tools/ci/convergence.py run "$contract" links` | PASS, exit 0, stable source, receipt `attempt-000001`; underlying `python3 tools/ci/check_docs.py` printed `Markdown=52 local_links=306 broken=0 okf_issues=0`. Nix reported an ignored busy eval-cache warning. |
| `nix develop -c python3 tools/ci/convergence.py wiki scan --ci-sha "$(git rev-parse HEAD)" --summary` | Exit 0, clean source, 306 links, empty errors/broken links. 8 stale candidates, 3 historical, 3 unaffected. This scan does not certify semantics. |
| `git diff --check e92b5db HEAD` | Exit 0. Scope contained Markdown and the existing provenance JSON. No evidence file was removed. |
| Changed-page fragment and Git-object checks, separately from the file-only link checker | 43 local fragments and 26 full-SHA blob links resolved at this commit. All 14 selected index-to-source paths resolved. The review contains the exact read-only probes. |

The default shell lacked PyYAML (`nix develop -c python3 -c 'import yaml; print(yaml.__version__)'`, exit 1). Parsing each frontmatter document with `yq -o=json . -` from `nix shell --inputs-from . nixpkgs#yq-go` succeeded for all 11 headers and their local source resources. No dependency change was required.

Local Rust, virtual, generated-media, GUI and physical tests were **NOT RUN** for this documentation change. CI and the local media/Wayland summaries above retain their original identities. The two-host, public-output and recovery permissions in historical receipts belonged to those trials.

On the final documentation content, `python3 tools/ci/check_docs.py` printed `Markdown=52 local_links=312 broken=0 okf_issues=0`. The fragment/object probe resolved 44 local fragments and 29 pinned source links. The 14 routes resolved again. `git diff --check e92b5db` and the diff restricted to paths outside the documentation scope were empty. The added-line scan found zero private path, access token, bearer value or private key patterns. The exact scripts and both reader responses accompany the draft review for replay on its head. `MEMORY.md` shrank from 1,599 bytes / 209 whitespace words to 1,193 / 168. The pull index grew from 2,478 / 272 to 3,353 / 350. Word counts are size proxies.
