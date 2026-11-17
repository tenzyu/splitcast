# Finite fixture playback reference

`splitcast poc-play --input FILE.wav --target PIPEWIRE_NODE` plays one local
fixture through the existing PipeWire player. It does not read Twitch, synthesize
speech, transfer audio between hosts, start Agent, or establish physical listening.
The command needs explicit permission to play on its selected host and device.

The input is a regular, non-symlink file with at most 4 MiB of bounded PCM16 WAV.
Supported audio has one or two channels, a sample rate from 8 to 48 kHz, and at
most 30 seconds of samples. Malformed or ambiguous chunks and inconsistent PCM
headers are rejected before a player starts. The format chunk must precede the
audio data. Padded ancillary chunks may precede the format chunk. The caller's input remains unchanged.

The target is an explicit node name, not a numeric object ID or serial. There is
no default-sink selection or route mutation.
The player receives validated raw PCM on stdin with its exact rate and channel
count. Fixed no-fallback and no-reconnect properties keep a missing destination
from selecting another sink. Native policy behavior requires its own device-free
verification and does not follow from an argv test alone.

The operation has a finite safety deadline and no playback retry. Cancellation
stops only its owned player group using the existing process owner. Process
teardown has its documented extra grace/reaping time; this is not a hard acoustic
cutoff or a universal guarantee for kernel-stuck or escaped descendants. A failed
or partially completed playback is not automatically repeated.

The JSON receipt identifies a local fixture and successful player-process
completion. Listening remains unverified. The operation creates no playback
files or recordings, and does not delete the supplied input. An enclosing trial
owns inspection and deletion of recordings that trial created.

Prepared Agents also use this PCM validator and player for session-scoped returns.
See the [Agent return protocol and evidence](evidence/audio-return.md). EventSub
and remote mixing remain gated.
See [the PoC checkpoint](evidence/poc-20261117.md) for actual execution and limits.
