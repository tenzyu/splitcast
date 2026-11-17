# Publish the standalone repository

The initial status and creation recipe below belong to the extraction checkpoint. Local status now shows `origin/main`, but this planning pass did not query the live repository or authentication. Inspect the existing remote before executing any creation command. The current next-session procedure is [the CI/cache handoff](https://github.com/tenzyu/splitcast/blob/archive/pre-dreaming-20261117/docs/handoff/ci-nix-cache.md).

S18 subsequently confirmed the existing public `tenzyu/splitcast` repository and
authenticated access. Do not run the repository-creation recipe against it.
The operator approved signed GitHub Releases delivery with corresponding sources
and an ordinary merge after validation. [The CI/cache record](https://github.com/tenzyu/splitcast/blob/archive/pre-dreaming-20261117/docs/evidence/ci-nix-cache-20261114.md)
retains the exact approval scope and actual publication/consumption results.
This does not authorize rewriting existing history or enabling a host service.

At the extraction checkpoint, the agent reported no authenticated GitHub CLI session and the system-configuration consumer used a pinned local Git flake input. Neither authentication nor the live consumer input was rechecked in this planning pass. The local repository has its own `main` history. Do not invent a public repository URL, claim publication, or assume the extraction checkpoint still describes current access.

Before publication, inspect `git status`, the semantic review, the verification record, and `THIRD_PARTY.md`. Confirm the right to publish the imported documentation and the two operator-supplied policies. Original SplitCast code is MIT; the OBS patch and historical copy have GPL-2.0-or-later sidecars and retained upstream license text. Generated recordings, private captures, credentials, build residue and raw local command logs remain ignored under `artifacts/`.

The current tree generalizes operational accounts as well as machine labels. Seven historical files in earlier standalone commits still contain source-account identifiers. Current-tree normalization does not anonymize Git history. The audit found no legacy machine aliases or selected credential signatures in the inspected commits, but the owner must still review this intentional historical disclosure. If it is unwanted, prepare a separate clean publication history while preserving the existing private history; do not force-push a public branch or silently rewrite evidence.

From the standalone repository, the authenticated owner can run:

```sh
gh repo create splitcast --public --source . --remote origin --push
git ls-remote origin refs/heads/main
```

If the repository already exists, inspect it before adding or changing a remote. Do not force-push or overwrite another repository. Compare the remote main SHA with local HEAD before reporting publication.

After the repository exists, update only the consumer's SplitCast input URL to `github:OWNER/splitcast`, replacing OWNER with the actual GitHub owner. Run `nix flake update splitcast`, evaluate the actual consumer gates, and review and commit that consumer diff. The local Git bootstrap is not a portable permanent URL for another machine.

The standalone `nix/consumer-tests.nix` accepts `{ consumer }` and validates the configured capture/production role pair without embedding their machine names. Evaluate it against the consumer's flake and require every returned check and `passed` to be true. Full consumer flake evaluation has a separate baseline issue documented in `docs/verification.md`; do not silently call that check passed.

Source publication does not authorize operating-system activation, physical capture, a privileged helper, external listeners, or public streaming. Keep the runtime gate false until the separate acceptance work is complete.
