# SplitCast memory

Keep osu! and its local game audio independent of streaming. The capture host sends selected game A/V and microphone audio. The optional camera can live on either role. The production host owns RNNoise, OBS and EventSub/VOICEVOX return to a selected capture-host sink. Private recording is the default. Physical capture, new OAuth, public output or listeners, privileged helpers and host activation each need current operator authority. Keep `rustCandidate=false` and services without autostart targets.

Start at the [topic index](docs/memory/index.md). Select the question, then inspect its exact revision and receipt. The [physical record](docs/memory/session-state.md) identifies the two-host recording, public archive decode and separate speech trial. NVIDIA OBS, gameplay p99 and physical A/V timing remain open. An open PR or a detached fixture run can change what to test next without changing the accepted host result.

Use the [verification record](docs/verification.md) for commands and failures. In an already active pinned project shell run the command directly. Otherwise use `nix develop -c cargo run -p splitcast-xtask -- verify` for the ordinary verifier.
