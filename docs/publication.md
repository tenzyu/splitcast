# Publish the standalone repository

The agent has no authenticated GitHub CLI session and must not invent a public repository URL or claim publication. The local repository has its own `main` history. The system-configuration consumer uses a pinned local Git flake input until the repository owner completes this step.

Before publication, inspect `git status`, the semantic review, the verification record, and `THIRD_PARTY.md`. Confirm the right to publish the imported documentation and the two operator-supplied policies. Original SplitCast code is MIT; the OBS patch and historical copy have GPL-2.0-or-later sidecars and retained upstream license text. Generated recordings, private captures, credentials, build residue and raw local command logs remain ignored under `artifacts/`.

From the standalone repository, the authenticated owner can run:

```sh
gh repo create splitcast --public --source . --remote origin --push
git ls-remote origin refs/heads/main
```

If the repository already exists, inspect it before adding or changing a remote. Do not force-push or overwrite another repository. Compare the remote main SHA with local HEAD before reporting publication.

After the repository exists, update only the consumer's SplitCast input URL to `github:OWNER/splitcast`, replacing OWNER with the actual GitHub owner. Run `nix flake update splitcast`, evaluate the actual consumer gates, and review and commit that consumer diff. The local Git bootstrap is not a portable permanent URL for another machine.

The standalone `nix/consumer-tests.nix` accepts `{ consumer }` and validates the configured capture/production role pair without embedding their machine names. Evaluate it against the consumer's flake and require every returned check and `passed` to be true. Full consumer flake evaluation has a separate baseline issue documented in `docs/verification.md`; do not silently call that check passed.

Source publication does not authorize operating-system activation, physical capture, a privileged helper, external listeners, or public streaming. Keep the runtime gate false until the separate acceptance work is complete.
