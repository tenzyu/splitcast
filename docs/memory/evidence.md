# Evidence and source register

Recorded 2026-11-13. Imported reports were read for this memory. Historical tests were not rerun. Source Git object IDs were read locally without contacting hosts or GitHub.

## Repository and artifact identities

| ID | Identity | Meaning |
| --- | --- | --- |
| SC-MONO | Source monorepo | Historical implementation and original Git evidence. No personal checkout path is required. |
| SC-STANDALONE | This standalone repository | Extraction target. Its results must have a new artifact identity. |
| BASE-PY | `6ceeb53b0d1e267fa01f04c94fdb6226877c93cf` in SC-MONO | Pre-retirement reference and historical development oracle, not an installed fallback. |
| REPAIR | `0fbad465122f538eda746704e151dff51c7d6102` in SC-MONO | Historical authority, microphone, camera, and observation repairs. |
| SHIPPED | `21a4f8828f326af1eaa28c4a0c56fef03874296f` in SC-MONO | Published partial-scope report, including REPAIR. Commit subject describes documentation, not a new implementation by itself. |
| EXTRACT | `3c8857353581424e5cca517d7ccf0db8de7514d4` in SC-MONO | Operator-supplied clean main, equal to origin/main, selected for extraction. This is not an acceptance run. |

The historical run used an isolated work branch and separate running NixOS generations. Neither those generations nor EXTRACT establish the active state of a host today.

## Source register

Paths are relative to SC-MONO. Local copies are under `docs/history/source/` with the same suffix unless normalization changes the filename. Git blob IDs identify the originals even when copies are sanitized. They are not checksums of sanitized copies. No link below asserts that private raw recordings or logs are shipped here.

| ID | Date | Source role and location | Original blob at EXTRACT |
| --- | --- | --- | --- |
| S00 | 2026-11-13 | Current extraction task and exact knowledge instruction in `MEMORY.md`; operator communication | Not a Git blob. Fresh implementation evidence pending. |
| S02 | 2026-11-05 | Domain research: `docs/graphics-streaming/RESEARCH-2026-11-05.md` | `5ee177d5bf32f184f5d22ddeb585227d0b84bc81` |
| S03 | 2026-11-06 | Initial approved integration decisions: `docs/graphics-streaming/DECISIONS-2026-11-06.md` | `a321bc0814dd5473442235c3b42bd83474915cce` |
| S04 | 2026-11-07 | Proposed performance-first target and audio amendment: `docs/graphics-streaming/ADR-2026-11-07-performance-first.md` | `1aa0915dc7237fdb7e8cb3b05acb04a8a6afc0f4` |
| S05 | 2026-11-13 | Later operator requirements: `memo.md` | `4d7ebc589b5f3bcadfb410fae925fa2d44a505b2` |
| S06 | 2026-11-13 | Consolidated implementation, test, and approval report: `docs/graphics-streaming/rust-completion-20261113.md` | `44f294981df1d671da755e2076bf6f9795061586` |
| S07 | 2026-11-13 | Adopted and rejected implementation shapes: `docs/graphics-streaming/wiki/design.md` | `09cfa687a2fd269392ed67517ab8cb647c59f193` |
| S08 | 2026-11-13 | Synthetic two-host protocol and limitations: `docs/evidence/splitcast-synthetic-ssh-20261113.md` | `a742958db97da984a31eb3ff5aac560876734b63` |
| S09 | 2026-11-13 | Selected actual red and green output: `docs/evidence/splitcast-rust-regressions-20261113.txt` | `2b9b7c3bd4b6bdf98be15bfc7deca422b820ee53` |
| S10 | 2026-11-13 | Append-only session decision trail: `.audit/splitcast-rust-completion.tsv` | `1de57f3c6a0e7ab3145402c8f811f74ab505413b` |
| S11 | 2026-11-07 | Observation register: `docs/graphics-streaming/EVIDENCE-2026-11-07.tsv` | `6273c5b431857f8ab38c1be1c18096717aee09ac` |
| S12 | 2026-11-02, retained in EXTRACT | Retired Python acceptance history: `docs/E2E_RESULTS.md` | `cf0f7bf34052e42aa4f5d53a40be70fc1e0f4df1` |
| S13 | 2026-11-01 onward, retained in EXTRACT | Earlier architecture decisions: `docs/ARCHITECTURE_DECISIONS.md` | `4f2e462b4f775c55279d69a48009e6c06cd904f9` |
| S14 | 2026-11-14 | Fresh standalone verification and normalized durable outputs | [Verification](../verification.md), `docs/evidence/standalone-20261114-summary.tsv` |
| S15 | 2026-11-13 through 2026-11-14 | Independent semantic review of this extraction | [Design review](../design-review.md) |
| S16 | 2026-11-14 JST | Virtual software implementation, failures, local reruns, and independent runtime probes at `aa86fe0` plus task changes | [Virtual verification record](../evidence/virtual-system-20261114.md), [design](../virtual-control-design.md), `.audit/virtual-system.tsv` |
| S17 | 2026-11-14 JST | Operator-requested memory consolidation, production-readiness recommendation, and next-session CI/cache handoff; source inspection only | [Working state](session-state.md), [readiness gates](../production-readiness.md), [handoff](../handoff/ci-nix-cache.md) |

Original object inspection uses `git ls-tree -r <EXTRACT> -- <source-path>` and `git cat-file -p <blob>` in SC-MONO. Those commands do not run the historical checks.

## Evidence register

| ID | Date and artifact | Observation | Confidence and boundary |
| --- | --- | --- | --- |
| E01 | 2026-11-07, historical capture-host runtime | Eight foreground Direct Scanout samples were active. PipeWire driver quantum later showed 256/48000 while a follower advertised 441/44100. | High confidence in S11's reported observations. No capture-on run, per-frame guarantee, physical latency result, or current-host claim. |
| E02 | 2026-11-13, historical repaired native implementation, summarized at SHIPPED | 102 ordinary Cargo tests passed. Two opt-in targets were ignored by default. Warning-denied all-target Clippy passed. Private-audio opt-in and empty-PATH native HTTP checks passed separately. | High documentary confidence from S06, S09, S10. Separate protocol and web-model results must not be added to the 102 without checking their suite identities. Not rerun here. |
| E03 | 2026-11-13, historical actual binary with stub workers and a loopback peer | Independent follow-up passed all 19 integration tests after REPAIR and rechecked disabled deployment gates. | S06 reports independent confirmation; S09 retains selected ordinary-suite output. The 19 are not automatically 19 additional unique tests beyond E02. No real capture or OBS acceptance. |
| E04 | 2026-11-13, historical repaired Nix artifact | Rust/WASM package and configuration check built. Installed selfcheck and example validation passed. Offline Doctor returned `ok=false`. | High documentary confidence from S06, S10. Build produced store artifacts, not activation. Doctor refusal is evidence against readiness, not a passing live preflight. |
| E05 | 2026-11-13, separate synthetic SSH harness | Generated 320×180, 15 fps H.264/AAC produced a 600.022-second recording, 9,000 video packets, and 28,126 audio packets. Full decode passed. Disconnect teardown and absence of owned cgroups were checked. | S08, S09. Real two-host transport, but no agent, GSR, OBS, game, microphone, or camera. Independent decoding reread an existing file, not another capture run. |
| E06 | 2026-11-13, initial EXTRACT to SC-STANDALONE checkpoint | Extraction baseline supplied in S00. Historical source documents and original blob identities were inspected for memory. | At this checkpoint all fresh standalone checks were PENDING. E07–E10 supersede that pending status only for their explicitly tested scopes. |
| E07 | 2026-11-14, SC-STANDALONE code imported in `a5a0a09` | Full verifier passed fmt, workspace/all-target check and Clippy, 110 ordinary tests, doctests, the no-default-features matrix, all-features check, WASM check, dependency trees, and portability. | S14 and the committed command summary. Three opt-in tests were ignored by default. Zero doctests exist. This is not a live media pass or a minimum-compiler search. |
| E08 | 2026-11-14, separate SC-STANDALONE local components | The real private Pulse server ownership test passed. The generated-only two-second H.264/AAC recording passed stream/packet/duration checks and full decode. | S14. No physical input, game, GSR, remote host, production OBS, or GPU path was exercised. These two passes are separate opt-in runs, not additional unique ordinary-suite tests. |
| E09 | 2026-11-14, SC-STANDALONE flake and actual consumer | Rust/WASM artifacts built; standalone flake checks passed. All 22 real module-evaluator assertions and all five actual consumer gate assertions passed. Capture disabled; production integrated but runtime gated and no autostart. | S14 and committed role-only JSON evidence. The module harness injects a synthetic package; the actual consumer evaluator separately checks the real external package identity. These are configuration checks, not activation. |
| E10 | 2026-11-14, SC-STANDALONE supply chain and source preservation | Cargo audit reported zero known vulnerabilities against database commit `4ef2df8e806a65e2f1ecc14f56f51249062b23d6`; cargo deny passed with duplicate-version warnings. Independent audit verified all 124 original Git blob/content identities and coverage of all 120 transferred tracked deletions. | S14. Advisory checks are not a security proof. Publication copies deliberately differ from originals; their current hashes are in the manifest. No GitHub publication occurred. |
| E11 | 2026-11-02, retired Python implementation, documentary only | Isolated and synthetic OBS recovery had failures and some later passes. Normal-direction synthetic recording passed once but leaked owned SSH processes requiring manual cleanup. | S12. These results neither contradict the current Rust path's unaccepted OBS gate nor establish current reliability. Keep environment, language implementation, date, direction and cleanup outcome attached to every reused claim. |
| E12 | 2026-11-14 JST, SC-STANDALONE `aa86fe0` plus virtual-control changes | Ordinary verifier passed 11 steps. Final extended verifier passed 14 steps, 124 ordinary tests, and three separate local opt-in components. Three integrated independent runtime reviews returned no verified scoped defects. | S16 and its source manifest. Actual local agents use stand-in external workers; generated media uses a separate delayed TCP path. No real SSH, actual agent RTMP or RTSP decode, physical capture, gameplay baseline, browser acceptance, Nix package rebuild, or deployment is implied. |
| E13 | 2026-11-14 JST, documentation-only planning at the same dirty source | Local Git/source inspection found the prior implementation uncommitted, no workflow files, and no project-declared binary-cache configuration. The readiness and CI/cache plans were written; checks are in [the handoff validation record](../evidence/handoff-20261114.md). | S17. No application matrix, Nix package build, hosted workflow, live GitHub/authentication inspection, cache upload, cache-consumption trial, or target-environment acceptance was run by this planning pass. E12 retains the actual local test evidence. |

## Corrections that must survive summarization

- S10's earlier 90-test row is not the final count. S09's final section and S06 report 102 ordinary tests. Keep each count tied to its run.
- Initial SSH authentication denial was superseded after an operator-installed restricted key enabled the authorized checks. KMS remained blocked, and that task-specific authorization was later removed. Neither result describes current access.
- Earlier independent review did not cover subsequent changes. Later review accepted disabled partial publication after REPAIR, not production cutover.
- A missing C linker was an environment failure, not behavioral red evidence. Some camera-fixture failures and a multicall `sleep` fixture defect also did not prove the intended runtime regressions. S09 distinguishes them.
- A small graph quantum is not the ALSA hardware period, the BASS callback period, or physical hitsound latency. S04 and S11 explicitly reject that inference.
- The first actual consumer import failed because two role bridges independently imported an anonymous Home Manager module. Stable, version-specific module identity and a canonical NixOS module path fixed that integration defect; do not suppress duplicate declarations or remove the inactive role's options.
- Full consumer `nix flake check --no-build` fails in FirefoxColor settings because a theme derivation is invalid in the local store. The same error reproduces at untouched EXTRACT. This is a remaining whole-consumer validation failure, not a passing check and not an observed SplitCast regression. Targeted consumer configuration build and role gates pass separately.
