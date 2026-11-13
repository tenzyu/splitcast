# Historical source records

`source/` contains normalized publication copies of prior source-repository records. They are not current commands, deployment instructions, or standalone test results. Earlier Python, service, display, and host-specific instructions may be obsolete.

The extraction baseline is source commit `3c8857353581424e5cca517d7ccf0db8de7514d4`. The original implementation and reports remain in the source repository's Git history. `import-manifest.json` records original Git blob IDs, original SHA-256 values, and imported normalized-content SHA-256 values. Later edits to current code do not change an imported snapshot's historical identity.

Publication normalization replaces machine aliases with role labels and personal home directories with `<operator-home>`. This deliberately changes hashes. Source Git blobs are the references for raw originals. To inspect one, run `git cat-file blob <source_git_blob>` in a checkout that retains the source history. The standalone repository does not pretend to contain those original Git objects.

The source corpus includes historical failures, superseded plans, partial passes, and commands that are no longer executable. The memory register identifies which findings remain relevant. Private local logs and recordings are not included merely because a historical report mentions them.

Additional source documents can be imported only with their original object and content identities recorded. Preserve the source first; label normalization and corrections rather than silently rewriting evidence.
