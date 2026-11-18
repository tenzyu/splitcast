# PR 16 OBS rendering and shutdown evidence

## Source and prior failure

The 2026-11-18 JST production trial recorded real game media with OBS 32.1.2, reported 117 late renders out of 645 attempts (18.1%), finalized the MKV, then exited with SIGSEGV. The original [trial receipt](../memory/session-state.md) retains those results. A production kernel journal observation ties the trial's exit to a fault at address `0x148` in `libobs.so.30` at ELF address `0xac727`. The original core file was removed as part of the trial cleanup. The journal is the retained independent exit evidence.

The patched OBS 32.1.2 binary SHA-256 is `c21427f9d4fbead6a4aaa9718ccf0b95c5dcbb58c9524f17bfcf4941fe963879`. Its `libobs.so.30` SHA-256 is `9590dfe552a6dbf49e3747c6d0d02534583e01024a78004c8c43b9222e5e3c1a`. With this library, `addr2line -a -f -C -e "$libobs" 0xac727` identifies `obs_enter_graphics`. The faulting instruction reads `0x148(%rax)` immediately after loading the global `obs` pointer. The kernel fault address and the instruction establish a null core dereference. The upstream caller remains unknown.

The upstream Linux SIGINT handler saves the scene and calls the main window's `close()`. SIGTERM saves the scene and calls `quit()`. Source inspection and the controlled signal comparison below establish different teardown paths. [OBS 32.1.2 signal handlers](https://github.com/obsproject/obs-studio/blob/32.1.2/frontend/OBSApp.cpp#L1792-L1862)

## Controlled production-host recording

Two direct child OBS processes used the same pinned package, private Xvfb with Mesa llvmpipe, x264, and the same generated 1364×768 H.264/AAC clip at 60 frames per second. The namespace exposed loopback only and hid graphics, microphone and camera devices. Each authenticated WebSocket session requested StopRecord, verified inactive output, closed its connection, and delivered one signal directly to the owned OBS process. The parent obtained the child wait status. Independent FFprobe and EOF decode verified both files. OBS ran directly without an Agent, Twitch configuration or GNU timeout wrapper.

| Signal after output inactive | Render stalls after warmup | OBS video-output skipped frames | Independent decode | OBS exit |
| --- | --- | --- | --- | --- |
| SIGTERM | 74/600 (12.3%) | 0/601 | 767/767 distinct video frames and 597 AAC frames, 12.834 s | SIGSEGV after 265 ms, same kernel fault address and instruction as the original trial |
| SIGINT | 69/601 (11.5%) | 0/601 | 767/767 distinct video frames and 597 AAC frames, 12.800 s | Exit code 0 after 215 ms |

Both OBS WebSocket listeners closed. The SIGTERM run independently reproduces the crash after output finalization with the renderer, encoder and shutdown path in isolation. A fixed process deadline, Twitch output and a live capture device are unnecessary for this reproduction. The different signal path resolved the reproduced shutdown failure in the direct SIGINT run.

There were ten one-second samples after two seconds of warmup in each arm. OBS's sampled average render times ranged from 16.81 to 23.25 ms for SIGTERM and 17.86 to 20.03 ms for SIGINT. The render stall rate exceeded 1% in both arms while the encoder-side video-output skip count stayed at zero. The single pair had different host load averages. Frame-level uncertainty, live workload regression, and physical media latency remain unmeasured.

The lead command was `nix develop --no-write-lock-file .#media-test -c bash artifacts/mission/obs-review/launch.sh software`. It exited 1 because the comparison retained the SIGTERM crash. Both child wait statuses, sample series, decoded file hashes and tool identities are under private receipt `artifacts/mission/obs-review/software-TrseFjVT`. Its diagnostic script SHA-256 is `1c2f54174cb76cf35cf66370107d086fc88df04f7a58899f581838689505464b`. Earlier failures remain in `software-K4v97joc` (Xvfb keymap startup, missing shell mount) and `software-DTLooJeG` (premature WebSocket readiness and Stop observation). Generated recordings and temporary profiles were removed after inspection. The earlier startup failures are outside the matched signal comparison.

## NVIDIA path

The production host exposed an RTX 4060 Ti with driver 595.71.05. The repository's NVIDIA Xorg service was absent from the loaded system and no existing display was available. The unprivileged diagnostic tried a private, loopback-only Weston headless OpenGL compositor with only the NVIDIA GPU devices and render node mapped. This kept the KMS primary node, physical audio/video devices and other sessions outside the namespace. The configured renderer reported NVIDIA on the RTX 4060 Ti. This establishes hardware rendering in the compositor.

OBS's Wayland EGL initialization failed with `eglGetDisplay failed`. Adding the installed NVIDIA external platform configuration made Weston report its Wayland EGL extension, but OBS still failed to initialize. The private Xwayland comparison reached X11, but Xwayland reported missing `wl_drm` and disabled Glamor and DRI3. OBS then failed its EGL/X11 display setup. Neither path reached OBS output. The same pinned package's `obs-nvenc-test` succeeded with CUDA 13.20, NVENC API 13.0, one compatible device and H.264 support. That capability probe opens an encode session and queries support. The OBS render-to-NVENC recording path and hardware input decode remain unmeasured.

Commands were `nix develop --no-write-lock-file .#media-test -c bash artifacts/mission/obs-review/launch.sh gpu` and the same command with `gpu-xwayland`. All returned 1. Private receipts are `gpu-wTMu3T9z`, `gpu-XZYFJwch`, `gpu-xwayland-Yv7rTdLI` (private X socket directory missing) and `gpu-xwayland-NvXaDqej`. The failed OBS startups required process-group kill. Their namespaces ended, the host had zero X11 socket files in its socket directory, the GPU's last sampled rendering/encoding/decoding utilization returned to zero, and the system display service remained unloaded. No privileged display was activated and no operating-system generation changed.

## Managed Agent generated recording

`nix develop --no-write-lock-file .#media-test -c bash tools/verify-media.sh --case normal --case camera` ran 16 media oracle unit tests and both selected device-free cases on the production host. The command returned 1 by design for a partial matrix. Both cases passed with the pinned production OBS package, Agent-created MediaMTX, RNNoise and private Pulse, and generated sources. The Agent recorded OBS exit code 0, output inactive and no termination signal. The harness separately verified the exact OBS process was gone, its WebSocket listener was closed, the private profile was removed, and the recordings decoded to EOF. The harness retains an open descriptor for the owned OBS log during Stop so it can preserve the shutdown messages after profile removal.

| Case | Decoded output at 1280×720/30 fps | Output log rendering lag | Cleanup |
| --- | --- | --- | --- |
| Normal | 355 video frames over 11.866 s. Both tone identities present in all 117 complete analysis windows | 3/393 attempted (0.8%) | OBS process and listener gone. Zero cleanup errors |
| Camera composition | 360 video frames over 12.033 s. Both tone identities present in all 119 complete analysis windows. Decoded inset passed its color-marker assertion | Zero lag line reported, 398 frames drawn | OBS process and listener gone. Zero cleanup errors |

The output log totals describe the 30 fps fixture. The 60 fps post-warmup production target and camera motion freshness remain open. The private run receipt is `artifacts/media-plane/runs/20261118T032417Z-KZVOhzn5`. It binds the dirty-tree source manifest `f79c132caf117d45a1abc86f167b59f8cb4eee51edd307bd6f2dd15996a1204d` over `0823165ecb6f5ecfe6d3a1b8df989bc62c80dfb0`, Agent binary SHA-256 `3e7c422f58e5dc3c99ed35c244e0aa99b0569f91e72578f100b3febd196c4545`, OBS closure inventory SHA-256 `be96d04d6f1e4925dcbd924e9446efdf8e7d33db34b5db1ecaf7bd39abefe712`, and unchanged source throughout the run.

Production hardware acceptance remains **BLOCKED** pending a usable isolated NVIDIA display for OBS and a source-bound run on the prescribed live inputs. The signal comparison reproduces the observed null core fault, and the managed generated runs establish a clean close at that source snapshot. The final Agent, physical production workload, sustained private Twitch test and viewer-side public playback each have their own acceptance gate.
