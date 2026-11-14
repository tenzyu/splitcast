# CI and release delivery

- [x] Preserve the existing verified source and handoff in a commit.
- [x] Implement read-only CI and start hosted verification.
- [x] Ground the existing packaging and release trust boundaries.
- [x] Compare independent release-cache sketches and select one.
- [x] Implement the selected publisher and a fresh no-build consumer.
- [x] Verify scripts, failure controls, hosted jobs and actual release consumption.
- [x] Reconcile records and review the decision trail.

Design rubric: exact native/WASM identities and closure coverage; PR/secret
isolation; immutable releases and retries; redistribution obligations; small,
rerunnable interfaces with negative controls. User approved a signed GitHub
Releases cache, signing-key creation and repository-secret configuration, and a
verification prerelease after redistribution checks. No deployment is approved.
