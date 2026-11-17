# Historical records and pruning manifest

Prior normalized source copies and overwritten long-form reports remain recoverable, byte-for-byte, at branch [`archive/pre-dreaming-20261117`](https://github.com/tenzyu/splitcast/tree/archive/pre-dreaming-20261117) (snapshot of original main `ed7fe60`). Git history preserves all previous branch edits. This branch is **historical evidence, not current instructions**.

## Preserved in current checkout

- [ADR-2026-11-07-performance-first.md](source/docs/graphics-streaming/ADR-2026-11-07-performance-first.md) — original architecture/performance ADR (unchanged).
- [ARCHITECTURE_DECISIONS.md](source/docs/ARCHITECTURE_DECISIONS.md) and [DECISIONS-2026-11-06.md](source/docs/graphics-streaming/DECISIONS-2026-11-06.md) — original rationale, historical environment and rejected alternatives (unchanged).
- `import-manifest.json` — **historical** source Git blob identities and normalized import hashes; it describes the original extraction, **not** a promise that every imported Markdown remains present at HEAD.
- Original non-Markdown source artifacts, logs, manifests and code copies not selected for Markdown pruning.

## Material archived from HEAD

The pruned Markdown paths are listed exactly in the [pre-Dreaming source tree](https://github.com/tenzyu/splitcast/tree/archive/pre-dreaming-20261117/docs/history/source). A request for a deleted path must use the archived branch URL or `git show archive/pre-dreaming-20261117:<path>`. Do not recreate full historical copies in the hot Wiki. Old Python deployment instructions and hardware observations are not valid current status.

The extracted corpus was normalized for publication: machine aliases, users and personal paths may differ from original source blobs; the import manifest distinguishes those hashes. Private raw logs and credentials were never authorized for publication.

For currently adopted choices follow [D01–D18](../memory/decisions.md); for execution/acceptance follow [PoC handoff](../handoff/poc-20261117.md). Never promote archival logs or a commit merge into contemporary test success.
