# Standalone verification

This record belongs to the extraction workspace, not to the historical source report. Source baseline is `3c8857353581424e5cca517d7ccf0db8de7514d4`. The standalone repository is initialized on `main`, without a remote. No GitHub operation, physical capture, privileged helper installation, service activation, or public publication has been performed.

## Executed local checks

`nix develop -c cargo run -p splitcast-xtask -- verify` runs and records the declared local matrix. The initial workspace run passed native checks, tests, doctests, the no-default-features matrix, the valid all-features check, WASM target check, dependency trees, and portability. Warning-denied Clippy exposed a pre-existing needless lifetime in the now directly checked protocol crate. The lifetime was elided without a behavioral change. A rerun passed the full verifier.

The ordinary workspace includes the 102 inherited native tests, three protocol tests, three native web-model tests, one remote-argument validation test, and one repository-portability test. This is 110 passing tests, not 110 additional tests beyond the inherited suite. Three targets are ignored by default: real private audio, generated local media, and two-host synthetic transport. Doctest targets currently contain zero examples; their successful runs do not imply documentation-example coverage.

The isolated private-audio opt-in passed. A new generated-only local FFmpeg/FFprobe test passed recording, both codecs, 30 video packets, positive audio packets, approximately two seconds duration, and full decode. It uses software codecs and temporary files, not GPU capture or physical devices. These command outputs remain in `artifacts/verification/private-audio.log` and `local-synthetic.log`.

`cargo audit --json` reported no known vulnerabilities against advisory database commit `4ef2df8e806a65e2f1ecc14f56f51249062b23d6`. This is not proof that dependencies are vulnerability-free. `cargo deny check` initially rejected unversioned internal path dependencies under the explicit wildcard policy. Exact matching internal versions were added instead of suppressing the policy. The subsequent run passed; duplicate-version warnings remain informational and are recorded.

Before Git initialization, the portability check found two additional machine aliases in historical Nix logs. Their publication copies were normalized; original source identities remain preserved. After Git initialization, one Nix invocation failed because the new flake was not yet staged. Staging the intended files repaired source filtering. Neither failure is behavioral red evidence for runtime correctness.

The verifier now retains each run under a distinct `artifacts/verification/runs/` directory and updates `latest-run.txt` as an index. Earlier failed logs remain intact. Fresh builds, consumer integration, final artifact hashes and independent review of this extraction are recorded below as they finish. Historical store paths and historical successes are not promoted to this run.

## Pending or unrun acceptance

The standalone Nix package build and consumer migration verification are pending at this checkpoint. Source integration is being changed to reference the independent flake; a local Git input is a bootstrap until the owner publishes the repository and updates its URL.

No real two-host SSH trial was run with the newly parameterized harness. No physical GSR, microphone, camera, OBS decode/preview/recording, browser interaction, forced-controller-death, filesystem fault injection, or game-performance baseline was run. Miri, fuzzing, sanitizers, lower compiler versions and other architectures have not been exercised.

The [semantic review](design-review.md) retains blocking runtime limitations in whole-server cancellation and task joining, accepted-request outcome ambiguity, and narrower API/FFI/browser weaknesses. Passing local tests do not close those findings. This remains a disabled partial candidate.
