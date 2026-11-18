# Studio control contract

The A-to-B checkpoint `df0768f` pinned the wire contract before
implementation. Pinned command names, arguments and response fields are
unchanged. The Agent owns the session and OBS effects.

## Transport and authority

Use authenticated `POST /api/command` on the existing loopback Agent, with
`Authorization: Bearer <local-control-token>`, `Content-Type: application/json`,
and exactly `{"command":"…","args":{…}}`. Use the existing Host/Origin rules
and finite request body and response framing. Browser clients keep the bearer
in memory. Never put it in a URL. Read legacy status with `GET /api/status`.
Existing Stop admission, `stop_id`, and completion observation still apply.

Production holds the chat and layout. A prepared capture Agent forwards all
seven Studio commands using its existing authenticated production forward and
current `_owner`; clients talking to capture omit `_owner`. Idle draft editing
and reload before Prepare require direct access to the production Agent.
Direct production reads can use the bearer without an owner. A supplied owner
must match current authority. When capture owns production, only the
matching owner may mutate or reconcile the scene. Idle local mutations use
the production bearer and omit `_owner`. Ownership does not replace bearer
authentication. A rejected mutation returns a non-2xx JSON error with
`outcome:"not_executed"`. A lost reply, a partial OBS write, or an uncertain
commit requires observing state before the client decides what to do next.

## Chat

Production accepts legacy `tts.eventsub = true` with
`tts.credentials_file`, or an independent `[chat]` table containing
`credentials_file` with the same private JSON credential format. Supplying
both sources is invalid. A production Agent can use chat with speech off.
Standalone production `prepare` with `{"media":false}` can start chat with
speech off. Capture's speech-only Prepare still requires speech return.
Existing TTS commands, IDs, consume-on-read, expiry, and the EventSub
counters under `tts` remain compatible.

Request:

```json
{"command":"chat","args":{"epoch":"0123456789abcdef0123456789abcdef","after":42}}
```

Response:

```json
{
  "enabled":true,
  "epoch":"0123456789abcdef0123456789abcdef",
  "cursor":43,
  "more":false,
  "reset":false,
  "overrun":false,
  "subscription":{"state":"subscribed","notifications":43,"duplicates":0,"dropped":0,"reconnects":0,"error":null},
  "messages":[{"cursor":43,"id":"chat-id","display_name":"viewer","text":"Hello!"}]
}
```

- Request with `{}` for the oldest retained page. Send `epoch` and `after`
  together for subsequent pages. `after` is exclusive. Cursors are exact
  JSON integers in the safe integer range. Feed the returned cursor back,
  including after an empty page. Read again when `more` is true, yielding
  to Stop and other actions between requests.
- The response returns at most 32 messages in ascending cursor order.
  The ring keeps the newest 128 distinct admitted messages. On same-epoch
  eviction past the requested cursor, `overrun:true` returns a page from the
  oldest retained entry. On epoch mismatch or cursor ahead of the current
  head, `reset:true` returns a page from the oldest retained entry. Both
  signal that the client must reset its view or mark a gap. An initial
  request reports neither flag. Idle production and disabled chat have
  `epoch:null`, `cursor:0`, `subscription:null`, and an empty page.
  `enabled` describes production configuration. A request carrying an epoch
  receives `reset:true` when there is no active history.
- History begins on each new Prepare and ends at session cleanup. It survives
  Twitch socket migration and subscription reconnect, and never backfills.
  A reconnect or revoked subscription can mean upstream loss even with a
  continuous cursor. The retained ID rule stays 4,096 IDs and ten minutes
  across reconnects within one Prepare. IDs are at most 128 ASCII bytes.
  Names are at most 128 UTF-8 bytes, text at most 512 UTF-8 bytes, both
  truncated at code point boundaries with control characters removed.
  A missing name is an empty string. The fields are untrusted plain text.
- A single subscription feeds the ring and an independent, existing bounded
  speech handoff when TTS is enabled. Slow or rejected speech can increment
  its own counters without removing chat history. Subscription `dropped`
  counts drops at the optional speech handoff. It does not count ring
  eviction. No additional scope, upstream connection or server polling.
- This is an immediate cursor delta, using one ordinary finite HTTP request
  and a short bounded snapshot. Consumers can piggyback reads on their
  visible chat refresh or request explicitly, with one in-flight delta per
  client and backoff when idle or contended. No hidden or always-on polling
  is required. A waiting HTTP subscription would hold scarce parser slots,
  and a persistent connection would require its own admission and cleanup.
  The delta uses neither. Stop keeps its independent priority admission.

## Scene

The layout is a complete document in OBS base-canvas pixels. In a live
session source dimensions come from OBS scene item transforms. Camera width
and height describe a fit-inside box, preserving the actual video aspect.
The editor uses the returned canvas and source geometry. Fixed GameFeed,
scene, audio routes, overlay font and z-order are Agent-owned.

```json
{
  "camera":{"x":952,"y":8,"width":320,"height":180,"visible":true},
  "overlay":{"text":"テスト中","x":16,"y":16}
}
```

The camera is `null` exactly when the production config omits `[camera]`.
Coordinates are non-negative integers, the camera box has positive extents
inside the canvas, and the overlay origin is inside the canvas. Overlay text
is null for absent, or a nonblank single line of up to 80 Unicode scalar
values. Null overlay uses the default origin (16, 16). Legacy
`obs.overlay_text` seeds the default layout, as do the existing camera
upper-right quarter-size box and former overlay bounds. A saved layout is
loaded from `studio-scene-v1.json` in the private production state
directory and takes precedence over the config defaults when valid. Config files are never rewritten.

| Command | Args | Meaning |
| --- | --- | --- |
| `scene` | `{}` | Read the Agent scene snapshot, including after a lost reply. |
| `scene-apply` | `{"generation":"<current>","operation":"edit-1","layout":{…}}` | Replace the in-memory draft while Idle or Ready, or apply and read back in a Running OBS session. |
| `scene-reconcile` | `{"generation":"<current>","operation":"readback-1"}` | Read actual owned Running OBS and update the observed layout, without OBS writes. |
| `scene-save` | `{"generation":"<current>","operation":"save-1"}` | Persist the current draft only after matching owned Running OBS readback. |
| `scene-reload` | `{"generation":"<current>","operation":"reload-1"}` | Reread saved state and reset the draft to the valid saved layout or config default. Live OBS is left as observed. |

All five return the same snapshot shape on success. For example, after
applying an edit with a configured camera:

```json
{
  "generation":"0123456789abcdef0123456789abcdef-2-5",
  "operation":{"id":"edit-1","state":"applied"},
  "canvas":{"width":1280,"height":720},
  "sources":{"game":{"width":1278,"height":720},"camera":{"width":320,"height":180}},
  "mode":"live",
  "draft":{"camera":{"x":952,"y":8,"width":320,"height":180,"visible":true},"overlay":{"text":"テスト中","x":16,"y":16}},
  "applied":{"camera":{"x":952,"y":8,"width":320,"height":180,"visible":true},"overlay":{"text":"テスト中","x":16,"y":16}},
  "saved":null,
  "persistence":"unsaved"
}
```

- `generation` is opaque and required on every mutating or reconciliation
  request. It advances on admitted scene operations and on lifecycle
  changes that invalidate the observation, including Stop and a new OBS
  session. Queue expiry, Stop and lease checks are the normal Agent fences.
  Two commands based on the same generation cannot both execute effects.
- `operation` is null or the last admitted scene operation in this lifecycle,
  with the client-supplied ID (1–128 ASCII alphanumerics, underscore or
  hyphen) and state `draft`, `applied`, `reconciled`, `saved`, `reloaded`,
  `failed`, or `unknown`. This is recent observation, not an idempotency key
  or durable journal. After a lost HTTP response get `scene` and correlate
  the operation and generation. A different operation or lifecycle can
  erase that proof. Never automatically replay a mutation.
- `mode` is `draft` in Idle or Ready and `live` in Running. Other lifecycle
  states reject scene controls. `sources` is null until a successful OBS
  readback. `sources.camera` is null without a configured camera.
  `applied` is null until verified. Start uses the current draft and begins
  a fresh OBS observation. Stop clears the live observation and keeps the
  draft in memory for explicit editing or a later Start. Drafts disappear
  when the Agent restarts unless saved.
- Apply keeps the requested draft. RPC rejection, a lost OBS reply or a
  mismatch can leave a mix of old and new properties. The Agent clears the
  live proof and reports an unknown operation. Reconcile reads OBS afresh
  without replaying writes. If the observation differs from the draft,
  neither is implicitly persisted. After observation the operator may
  explicitly submit a fresh edit with the new generation.
- Save verifies the current OBS layout again against the draft before
  atomically replacing the private layout file. `saved` is the last known
  file layout or null if absent or unknown. `persistence` is `saved` when
  the draft matches a confirmed saved file, `unsaved` when it does not, and
  `unknown` when persistence cannot be confirmed. Before-rename failure
  leaves the previous file in place. A directory sync failure after rename
  can leave a visible new file with unknown durability. Reload reconciles
  the visible file and confirms its directory when durability was uncertain.
  A malformed file puts persistence in the unknown state and leaves the
  current draft available. Saving a valid observed draft can replace the
  file once the underlying error is resolved. The save checks cancellation
  immediately before rename. An admitted filesystem commit can finish while
  Stop is pending, and cleanup completion follows the owner's return from
  that commit.

## Preview

Legacy `{"command":"preview","args":{}}` keeps its existing 960×540 PNG
request and `{"image":"data:image/png;base64,…"}` response. For Studio,
`{"command":"preview-small","args":{}}` uses the same response shape,
requests 480×270 from the owned Running OBS scene, and rejects a missing,
oversize or invalid PNG header and dimensions outside 480×270. The encoded
image is capped at 768 KiB. The command uses one bounded OBS screenshot RPC
over the existing authenticated, cancellation-aware path. Live scene operations
have a five-second OBS network budget from connection through readback. Idle or Ready
can edit a draft without OBS and cannot request a small screenshot. A client
can request a new snapshot explicitly after an edit. This contract has no
preview-only Start or continuously running preview workload.

The scene-item fields and screenshot aspect behavior follow the pinned
[OBS WebSocket protocol](https://github.com/obsproject/obs-websocket/blob/1fcb95b15aa88b1b7e9bda3f9c8650e314377169/docs/generated/protocol.md).
