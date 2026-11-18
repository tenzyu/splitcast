# S34 · SplitCast context retrieval in T3 Code

2026-11-18 JST. The unit under test was the agent reading this repository. The application, capture host and production host were outside the test.

## Source and discovery

The GitHub observation on November 18 found four open drafts. #18 `5485ca89`, #19 `51bd829d` and #21 `7e668a7a` targeted main. #20 `6fcec4a3` targeted #19's branch. These are dated snapshots. The independent readers worked offline. The tested main object was `e92b5db390835041b33de46618c49d5f98cad9cf`. The local Wiki scanner compares declared file dependencies. For live PR state, check the affected PR and its stack neighbor. [Snapshot correction procedure](../memory/update-protocol.md#when-a-proposal-moves)

| Surface | Observation |
| --- | --- |
| T3 runtime | T3 server and CLI 0.1.8, Codex CLI 0.169.1, NixOS x86-64. Fresh top-level threads ran with DisplayName Codex-2 (`codex_2`), `gpt-6.2-astra`, `reasoningEffort=xhigh`. T3 recorded the existing worktree `splitcast-context-skill`, branch `feat/wiki-context-skill`, as the thread cwd. |
| Codex catalog | A fresh stdio app-server accepted `initialize`, `initialized`, then `skills/list` with `forceReload: true` and both absolute cwds. The candidate returned one enabled `splitcast-context`, scope `repo`, path ending in `.agents/skills/splitcast-context/SKILL.md`. The main checkout returned zero. Both cwd entries reported zero errors. |
| Explicit invocation | A fresh thread received `$splitcast-context Which receipt would you use to distinguish a historic hosted Prepare failure from a local reproduction?` It opened the skill and the #19 verification receipt and distinguished the unknown hosted cause from the local collision. |
| Implicit invocation | The historical questions below did not name a skill. Both topical batches in each enabled arm announced the skill and opened its file. Each pair of unrelated controls completed with zero tool calls. |
| T3 composer | The operator selected the candidate worktree in another thread, typed `$`, and saw the skill listed as a repo skill. The automated preview failed on missing browser libraries. An isolated Nix Chromium reached the T3 pairing screen. Visual confirmation is operator-reported. |

The design keeps facts in the Wiki, loads the skill body on demand, and checks observed invocation independently of the composer. [Anthropic memory study](https://www.anthropic.com/research/components-of-agent-memory) · [Astra skills and AGENTS.md](https://developers.openai.com/blog/rethinking-skills-and-prompts-for-gpt-6-astra) · [OpenAI skill evaluation](https://developers.openai.com/blog/eval-skills) · [T3 picker cwd issue](https://github.com/pingdotgg/t3code/issues/6449) · [T3 Codex provider](https://github.com/pingdotgg/t3code/blob/main/docs/user/providers-codex.md)

## Matched reader inputs

Before adding the skill, select Q03, Q04, Q05, Q13 as one batch and Q06, Q07, Q08, Q10 as another. Use the exact [S33 questions and criteria](retrieval-questions.md). A third fresh thread gets two unrelated requests together: correct “the documet is ready” and convert three seconds to milliseconds. Each arm therefore answers ten items in three fresh threads. The baseline includes the full #21 retrieval improvements present at `7e668a7a`.

Both topical batches received the same four offline source pins, the same eight-shell-invocation allowance and a 100-word budget per answer. The prompt requested primary paths, pins, sections, symbols or jobs and allowed abstention. Each shell read ran inside an unprivileged, read-only Bubblewrap corpus at `/repo`, with isolated networking, a private temporary directory, and only the required tool closures mounted. The T3 thread itself retained the actual worktree cwd for skill discovery. Activity was checked for out-of-corpus reads.

To reconstruct the reader inputs, export #21 with `git archive`, excluding `docs/evidence/retrieval-questions.md` and `docs/evidence/dreaming-retrieval-20261118.md`. Initialize an independent Git object store in the export, fetch only the four primary commits and their ancestry, and set its HEAD/index to `e92b5db`. The working copy supplies the newer Wiki. Verify that neither excluded file nor the #21 commit object is readable. For an enabled arm, overlay only its diff from #21 before starting fresh readers. Keep reader outputs, scorer inputs and this receipt outside that mount. The draft review retains the submitted answers. No reader had GitHub, earlier transcripts, the rubric or the grader as an input.

Score factual completeness and primary-source support separately. The two controls earn factual points only. For the two earlier misses, check both the chat concurrency/backoff and preview cadence, and whether the queued generation stays invalid after Idle. A later pass adds an observation to the record for its own revision.

## Results

| Reader input | Correct / 10 | Primary support / 8 | Shell calls | Input tokens (cached subset) | Output tokens | Sum of reader seconds |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| #21 `7e668a7` | 8 | 8 | 11 | 426,692 (321,408) | 3,800 | 152.4 |
| First skill `f58d6df` | 9 | 8 | 14 | 530,464 (434,560) | 4,055 | 163.5 |
| Bounded lookup `594d05e` | 9 | 8 | 15 | 572,256 (477,568) | 4,640 | 187.4 |
| Final skill `e30d9de` | 9 | 8 | 13 | 510,500 (365,312) | 5,088 | 188.5 |

T3’s per-turn `turnTokenUsage` reported complete main-agent usage and the Astra model for each reader. Input includes repeated and cached context. Shell calls count actual command executions, excluding workspace setup. The three concurrent reader times are summed from provider start to completion. The longest individual reads were 81.5, 88.2, 105.1 and 100.5 seconds respectively. Graders and explicit-invocation probes are outside these totals.

| Question | #21 | First | Bounded | Final |
| --- | ---: | ---: | ---: | ---: |
| 03 · hosted 409 | 1 | 1 | 1 | 1 |
| 04 · Wayland readers | 0 | 0 | 1 | 1 |
| 05 · queued activation and Stop | 0 | 1 | 1 | 1 |
| 13 · squash and retarget | 1 | 1 | 1 | 1 |
| 06 · moving versus frozen content | 1 | 1 | 1 | 1 |
| 07 · render misses and hardware | 1 | 1 | 1 | 1 |
| 08 · Xorg recovery | 1 | 1 | 1 | 0 |
| 10 · sampled replay and physical phase | 1 | 1 | 0 | 1 |
| N · two negative controls | 2 | 2 | 2 | 2 |

The baseline described Stop fencing queued work but left its persistence after cleanup unstated. It omitted the chat and preview scheduling rules. The first skill reader gave the preview cap and chat delay range but omitted the one-in-flight rule and empty-page trigger. Its trace also tried to open a newer Wiki topic in the older main object. The next skill revision distinguished working-tree topics from pinned implementations. Both topical readers then searched for level-three headings where the Wiki used other depths. The final example accepts heading levels two through six.

The final answers recovered both rules and kept the hosted 409 cause unknown. Q08 lost one point for omitting Weston's successful NVIDIA rendering alongside the NVENC capability probe and OBS EGL failure. The reader preserved all three Xorg Stop timeouts, the operator reset and the boundary on another host trial. The intermediate Q10 answer said “within 1 ms,” which would also exclude an improvement larger than 1 ms. The final answer correctly gave the one-sided increase limit. The baseline also supplied one broken supplemental fixture link to a page absent at `e92b5db`. Its primary recovery receipt still supported the answer. The final adjudicator found zero false assertions, broken citations or safety/authority promotions in the delivered arm.

An independent Astra/XHigh scorer in a fresh Scratch thread received shuffled anonymous submissions from all four revisions, the S33 criteria, and the same offline primary sources for checking disputed terms. It could not see the condition mapping. An earlier rubric-only scorer on the first two arms gave 4/10 and 5/10, with primary support 8/8 each. It questioned whether the missing receiver log referred to the worker and whether reachability after squash was valid. The #19 receipt identifies the stand-in, and the correction protocol requires reachability for a **new provenance review** while retaining the original red job. The source-assisted scorer applied one set of criteria to all four arms. The difference between graders limits precision of the headline score. The operator requested GPT-6.2 Sol with Low reasoning for any future scoring of this panel. Record the scorer separately from the reader model. [#19 receipt](https://github.com/tenzyu/splitcast/blob/51bd829d56169b930d60315d2682f0c4c6eec2f9/docs/verification.md#L48-L71) · [Review ancestry rule](../memory/update-protocol.md#when-a-proposal-moves)

The delivered change gained one complete answer on this fixed ten-item panel at a cost of two more shell calls, 1,288 more output tokens and 36.1 additional summed reader seconds. Six calls served the incident batch in both the baseline and final runs. The media/hardware batch used five versus seven. The observed gain was on the two source routes that motivated the work. The result includes both skill and topic changes, a reused baseline, repeated questions and one observation per revision. General accuracy gains and reductions in unnecessary tool use remain unestablished.

## Verification and limits

Commands below ran in the candidate checkout. `direnv exec .` selected the reviewed project shell. Convergence contract `wiki-context-checks` pinned #21, the nine allowed files, and the following two executable checks.

| Command or request | Observed result |
| --- | --- |
| `direnv exec . python3 tools/ci/test_convergence.py WikiTests` (via `convergence.py run "$contract" wiki`) | 9 tests passed in 5.657 s at `f58d6df`, attempt-000001. Coverage includes stale and unrelated files, missing coverage, pinned CI identity, review ancestry, dry runs, failed proposals and worktree isolation. |
| `direnv exec . python3 tools/ci/check_docs.py` (via `convergence.py run "$contract" links`) | At `e30d9de`, 53 Markdown files, 318 local links, zero broken links or OKF issues. With S34 appended, 54 Markdown files and 324 local links passed with zero broken links or OKF issues. |
| `direnv exec . python3 tools/ci/convergence.py wiki scan --summary` and `direnv exec . python3 tools/ci/convergence.py wiki dream` | At `f58d6df`, 9 stale candidates, 3 historical, 2 unaffected, 11 unbaselined, zero link or scan errors. The final record also had 183 scanned local links and zero errors, with the same page counts. Dream returned PLAN_ONLY with 3 queued, 2 displayed. No proposal executed. |
| `rg -i -A 8 '^#{2,6} .*queued activation' docs/memory/open-risks.md` | Resolved the final skill example at the actual level-four heading and exposed the pinned regression. |
| `python3 check_routes.py` (probe in the draft review) | Resolved 17 local heading links and 22 pinned file/line/heading links across the skill and its three edited routing pages. The first probe incorrectly parsed a pinned Markdown heading as a line range and failed on `#finite-matched-acceptance`. The corrected probe handles both fragment forms. |
| `codex app-server --listen stdio://` | The cwd-paired `skills/list` probe returned the expected repo scope and clean-checkout control. The draft review includes the request. |
| Skill creator's `quick_validate.py .agents/skills/splitcast-context` | Printed `Skill is valid!` on the initial and final bodies. `python3 -c 'import yaml'` in the dev shell and `nix shell --inputs-from . nixpkgs#python3Packages.pyyaml -c python3 -c 'import yaml; print(yaml.__version__)'` initially failed with `ModuleNotFoundError`. The pinned package supplied the validator dependency using the command below. |
| `git diff --check 7e668a7` and `direnv exec . python3 tools/ci/convergence.py check "$contract"` | No whitespace or scope violations. The first local contract named an import-style unittest command before its sibling imports were inspected. The corrected contract used the module's script entrypoint. The original command was unrun. |

```sh
# In the approved dev shell, with the installed skill-creator directory as skill_creator.
pkg=$(nix build --no-link --print-out-paths --inputs-from . nixpkgs#python3Packages.pyyaml)
PYTHONPATH="$pkg/lib/python3.13/site-packages" python3 "$skill_creator/scripts/quick_validate.py" .agents/skills/splitcast-context
```

The primary-reference batch returned HTTP 405 in the web tool. Direct HTTPS fetched the two OpenAI posts, the Anthropic paper and T3 provider guide. The unauthenticated issue API returned 403. `gh api repos/pingdotgg/t3code/issues/6449` retrieved the closed picker issue. These references motivated the tests. The discovery and scoring observations above come from the installed environment.

MCP unnecessary; not added. The runtime exposed the skill, Git served the pinned sources and the existing Wiki scanner handled local provenance. No new CLI, MCP schema or service entered reader context. MCP runtime overhead was not benchmarked. Rust verification, operational fixtures, physical capture, output and deployment were outside this documentation and harness change. The application gates continue to use their existing receipts. [Physical status](../memory/session-state.md)
