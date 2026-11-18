# SplitCast Studio

Studio is an optional Linux desktop client for the [Studio control
contract](../../docs/studio-control-contract.md). The locked package is
separate from the Agent's Cargo workspace and default Nix outputs. It uses
Slint's Winit software renderer on Wayland or X11. The About panel includes
the attribution for Slint under its Royalty-Free License.

## Open Studio

Open Studio for an already-running capture Agent on local port 17870.

```sh
nix run .#splitcast-studio -- --port 17870 --token-file "$HOME/.config/splitcast/control.token"
```

Use the Agent's local control bearer in a private file owned by the current
user, with mode 0600. Studio only connects to IPv4 loopback. With an
operator-established loopback SSH forward, supply the local forwarded port
and the Agent's original listener port separately. This example uses local
port 28071 for production port 17871.

```sh
nix run .#splitcast-studio -- --port 28071 --agent-port 17871 --token-file "$HOME/.config/splitcast/production.token"
```

Use the production Agent directly to edit its draft in Idle. Use the capture
Agent to edit and observe production during a capture-owned session. Studio
stages edits locally in memory. Apply sends the current generation and the
complete draft. In Ready or Idle this updates the Agent's in-memory draft.
In Running it writes the layout to owned OBS and reads it back. OBS readback
lets the operator inspect OBS separately. Save requires a matching live
readback and persists through the Agent's existing state directory.
Reload saved updates the draft from that directory.

Recording, Test and Live select an output mode. Press Start separately after
Prepare and the current Ready observation. Live also needs an explicit
duration from 1 to 28800 seconds and the exact confirmation `LIVE`.
Production permissions remain authoritative.

Stop can be sent while another request is pending. Studio shows admission
and waits for the matching completion observation. On capture, that proof
includes the Agent's release of its remote ownership obligation. An unknown
result calls for inspecting Status or reading the Agent scene before issuing
another command. Closing Studio leaves the Agent session active.

The preview requests owned OBS snapshots in a Running session while Studio
is visible and both Preview and Scene Editor are shown. The display accepts
at most 480 × 270 pixels, at most once per second. The
camera and overlay outlines are draft placement guides. Confirmed
coordinates appear as OBS readback in the editor. Visible chat fetches one
cursor delta at a time, backs off on empty pages, and retains at most
128 rows. Both readers continue when another application has keyboard
focus. They pause when Studio is hidden, when the window backend reports
occlusion, or when their panels are closed. The pinned Winit backend does
not report compositor occlusion on Wayland.

## Local development

```sh
nix develop .#studio -c cargo test --manifest-path packages/splitcast-studio/Cargo.toml --locked
nix develop .#studio-wayland-test -c python3 packages/splitcast-studio/tests/wayland.py --evidence artifacts/studio-wayland/replay
nix develop .#studio-test -c python3 packages/splitcast-studio/tests/gui.py
```

The Wayland fixture launches its own headless Sway compositor with Pixman
and Xwayland disabled. It tests a focus handoff to a second native client
while both windows are visible, captures the chat region, and records
process PSS and CPU. Both GUI fixtures use isolated Agents and local
stand-in workers. The [verification record](../../docs/verification.md)
tracks the measured environment and gates.
