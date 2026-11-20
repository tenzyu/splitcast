pub use crate::{
    chat::Cursor as ChatCursor,
    scene::{Edit as SceneEdit, SceneLayout},
};
use serde_json::{json, Map, Value};
use std::io;
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Output {
    Recording,
    TwitchTest,
    TwitchLive,
}

impl Output {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Recording => "recording",
            Self::TwitchTest => "twitch-test",
            Self::TwitchLive => "twitch-live",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Start {
    output: Output,
    limit_seconds: Option<u64>,
    session_id: Option<String>,
}

impl Start {
    pub fn output(&self) -> Output {
        self.output
    }

    pub(crate) fn limit(&self) -> Option<Duration> {
        self.limit_seconds.map(Duration::from_secs)
    }

    pub(crate) fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    fn parse(args: &Map<String, Value>) -> io::Result<Self> {
        let output = match args
            .get("output")
            .map(|v| v.as_str().ok_or_else(invalid))
            .transpose()?
        {
            None | Some("recording") => Output::Recording,
            Some("twitch-test") => Output::TwitchTest,
            Some("twitch-live") => Output::TwitchLive,
            _ => return Err(invalid()),
        };
        let limit_seconds = args
            .get("duration_seconds")
            .map(|value| {
                value
                    .as_u64()
                    .filter(|seconds| (1..=28_800).contains(seconds))
                    .ok_or_else(invalid)
            })
            .transpose()?;
        if output == Output::TwitchLive {
            if args.get("confirm_public").and_then(Value::as_bool) != Some(true)
                || limit_seconds.is_none()
            {
                return Err(io::Error::new(io::ErrorKind::InvalidInput,
                    "public Start requires confirm_public=true and duration_seconds from 1 to 28800"));
            }
        } else if args.contains_key("confirm_public") {
            return Err(invalid());
        }
        let session_id = args
            .get("session_id")
            .map(|value| {
                value
                    .as_str()
                    .filter(|id| valid_id(id))
                    .map(str::to_owned)
                    .ok_or_else(invalid)
            })
            .transpose()?;
        Ok(Self {
            output,
            limit_seconds,
            session_id,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MicAction {
    Status,
    Mute,
    Unmute,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Status,
    Heartbeat,
    Doctor,
    Devices {
        remote: bool,
    },
    Prepare {
        media: bool,
    },
    Start(Start),
    Stop,
    Routes {
        media: bool,
    },
    Mic(MicAction),
    TtsStatus,
    Say {
        text: String,
        id: String,
    },
    Diagnostics,
    Preview,
    PreviewSmall,
    Chat(Option<ChatCursor>),
    Scene,
    SceneApply {
        edit: SceneEdit,
        layout: SceneLayout,
    },
    SceneReconcile(SceneEdit),
    SceneSave(SceneEdit),
    SceneReload(SceneEdit),
    TtsNext,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Invocation {
    pub command: Command,
    pub owner: Option<OwnerId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerId(String);
impl OwnerId {
    pub fn parse(owner: &str) -> io::Result<Self> {
        if owner.is_empty() || owner.len() > 128 || owner.chars().any(char::is_control) {
            Err(invalid())
        } else {
            Ok(Self(owner.to_owned()))
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn matches_prepare_id(&self, prepare_id: &str) -> bool {
        self.0
            .rsplit_once('-')
            .is_some_and(|(id, nonce)| id == prepare_id && valid_nonce(nonce))
    }
}

fn valid_nonce(nonce: &str) -> bool {
    nonce.len() == 32
        && nonce
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn instance_generation(id: &str) -> Option<(&str, u64)> {
    let (instance, digits) = id.rsplit_once('-')?;
    if !valid_nonce(instance) || digits.len() > 20 {
        return None;
    }
    let generation = digits.parse::<u64>().ok()?;
    (digits == generation.to_string()).then_some((instance, generation))
}

/// Capture's record of the production incarnation and Idle revision it used.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RemoteOwner {
    owner: OwnerId,
    instance: String,
    generation: u64,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct StopId(String);

/// Only a correlated production completion can construct a release proof.
#[derive(Debug)]
pub(crate) struct StopProof(RemoteOwner);

impl StopProof {
    pub(crate) fn proves(&self, owner: &RemoteOwner) -> bool {
        &self.0 == owner
    }
}

impl RemoteOwner {
    #[cfg(test)]
    pub(crate) const FIXTURE_INSTANCE: &str = "0123456789abcdef0123456789abcdef";

    #[cfg(test)]
    pub(crate) fn fixture(nonce: u128) -> Self {
        Self::from_prepare(
            &format!("{}-0-idle", Self::FIXTURE_INSTANCE),
            &format!("{nonce:032x}"),
        )
        .unwrap()
    }

    pub(crate) fn from_prepare(prepare_id: &str, nonce: &str) -> io::Result<Self> {
        let (instance, generation) = prepare_id
            .strip_suffix("-idle")
            .and_then(instance_generation)
            .filter(|_| valid_nonce(nonce))
            .ok_or_else(|| io::Error::other("invalid production Idle prepare ID or nonce"))?;
        Ok(Self {
            owner: OwnerId::parse(&format!("{prepare_id}-{nonce}"))?,
            instance: instance.to_owned(),
            generation,
        })
    }

    pub(crate) fn owner(&self) -> &OwnerId {
        &self.owner
    }

    fn stop_id<'a>(&self, document: &'a Value) -> io::Result<&'a str> {
        document["stop_id"]
            .as_str()
            .filter(|id| {
                instance_generation(id).is_some_and(|(instance, generation)| {
                    instance == self.instance && generation > self.generation
                })
            })
            .ok_or_else(|| {
                io::Error::other("remote Stop ticket did not match the held production instance")
            })
    }

    pub(crate) fn admitted_stop(&self, document: &Value) -> io::Result<StopId> {
        let id = self.stop_id(document)?;
        if !document["owner"].is_null() && !self.matches_owner(document) {
            return Err(io::Error::other(
                "remote Stop admission reported a different owner",
            ));
        }
        Ok(StopId(id.to_owned()))
    }

    fn matches_owner(&self, document: &Value) -> bool {
        document["owner"].as_str() == Some(self.owner.as_str())
    }

    pub(crate) fn observe_stop(
        &self,
        document: &Value,
        admitted: Option<&StopId>,
    ) -> io::Result<Option<StopProof>> {
        let id = self.stop_id(document)?;
        if !self.matches_owner(document) || admitted.is_some_and(|expected| expected.0 != id) {
            return Err(io::Error::other(
                "remote Stop observation changed owner or ticket",
            ));
        }
        if document["state"] == "idle"
            && document["stop"] == "completed"
            && document["outcome"] == "completed"
        {
            return Ok(Some(StopProof(self.clone())));
        }
        if admitted.is_some()
            && document["state"] == "stopping"
            && document["stop"] == "requested"
            && document["outcome"] == "accepted"
        {
            return Ok(None);
        }
        Err(io::Error::other(
            "remote Stop did not prove completion for the owned session",
        ))
    }
}

pub(crate) fn transport(cfg: &crate::config::Config, media: bool) -> Value {
    let mut contract = json!({"version":3});
    if media {
        contract["media"] = json!({
            "video":{"codec":"H264","width":cfg.width,"height":cfg.height,"fps":cfg.fps},
            "audio":{"game":"MPEG-4 Audio","mic":"Opus","rate":48000},
            "ports":{"rtmp":cfg.rtmp,"rtsp":cfg.rtsp}
        });
        if let Some(camera) = &cfg.camera {
            contract["media"]["camera"] = json!({
                "location":camera.location,"codec":"H264","width":camera.width,
                "height":camera.height,"fps":camera.fps
            });
        }
    }
    if cfg.tts_enabled {
        contract["speech_return"] = json!({"version":1,"expiry_seconds":cfg.tts_expiry_seconds});
    }
    contract
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "unknown or invalid command arguments",
    )
}
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

fn string<'a>(args: &'a Map<String, Value>, name: &str) -> io::Result<&'a str> {
    args.get(name).and_then(Value::as_str).ok_or_else(invalid)
}
fn action(args: &Map<String, Value>) -> io::Result<&str> {
    if args.contains_key("action") {
        string(args, "action")
    } else {
        Ok("status")
    }
}

impl Invocation {
    pub fn from_body(body: &str, max_text: usize) -> io::Result<Self> {
        if body.len() > 16384 {
            return Err(invalid());
        }
        let value: Value = serde_json::from_str(body).map_err(|_| invalid())?;
        let object = value.as_object().ok_or_else(invalid)?;
        if object.len() != 2 || !object.contains_key("command") || !object.contains_key("args") {
            return Err(invalid());
        }
        Self::parse(string(object, "command")?, &value["args"], max_text)
    }
    pub fn parse(name: &str, args: &Value, max_text: usize) -> io::Result<Self> {
        let args = args.as_object().ok_or_else(invalid)?;
        let allowed: &[&str] = match name {
            "status" | "heartbeat" | "doctor" | "stop" | "diagnostics" | "tts-next" | "preview"
            | "preview-small" | "scene" => &[],
            "devices" => &["remote"],
            "start" => &["output", "confirm_public", "duration_seconds", "session_id"],
            "prepare" | "routes" => &["media"],
            "mic" | "tts" => &["action"],
            "say" => &["text", "id"],
            "chat" => &["epoch", "after"],
            "scene-apply" => &["generation", "operation", "layout"],
            "scene-reconcile" | "scene-save" | "scene-reload" => &["generation", "operation"],
            _ => return Err(invalid()),
        };
        if args
            .keys()
            .any(|k| k != "_owner" && !allowed.contains(&k.as_str()))
        {
            return Err(invalid());
        }
        let owner = if args.contains_key("_owner") {
            let owner = string(args, "_owner")?;
            Some(OwnerId::parse(owner)?)
        } else {
            None
        };
        let text = |limit: usize, empty: bool| -> io::Result<String> {
            let text = string(args, "text")?;
            if (!empty && text.trim().is_empty())
                || text.chars().count() > limit
                || text
                    .chars()
                    .any(|c| c.is_control() && c != '\n' && c != '\t')
            {
                return Err(invalid());
            }
            Ok(text.to_owned())
        };
        let id = || -> io::Result<String> {
            let id = string(args, "id")?;
            if !valid_id(id) {
                return Err(invalid());
            }
            Ok(id.to_owned())
        };
        let media = || match args.get("media") {
            Some(value) => value.as_bool().ok_or_else(invalid),
            None => Ok(true),
        };
        let command = match name {
            "status" => Command::Status,
            "heartbeat" => Command::Heartbeat,
            "doctor" => Command::Doctor,
            "devices" => Command::Devices {
                remote: match args.get("remote") {
                    Some(v) => v.as_bool().ok_or_else(invalid)?,
                    None => true,
                },
            },
            "prepare" => Command::Prepare { media: media()? },
            "stop" => Command::Stop,
            "start" => Command::Start(Start::parse(args)?),
            "routes" => Command::Routes { media: media()? },
            "mic" => Command::Mic(match action(args)? {
                "status" => MicAction::Status,
                "mute" => MicAction::Mute,
                "unmute" => MicAction::Unmute,
                _ => return Err(invalid()),
            }),
            "tts" if action(args)? == "status" => Command::TtsStatus,
            "say" => Command::Say {
                text: text(max_text, false)?,
                id: id()?,
            },
            "diagnostics" => Command::Diagnostics,
            "preview" => Command::Preview,
            "preview-small" => Command::PreviewSmall,
            "chat" => Command::Chat(ChatCursor::parse(args)?),
            "scene" => Command::Scene,
            "scene-apply" => Command::SceneApply {
                edit: SceneEdit::parse(args)?,
                layout: SceneLayout::parse(args.get("layout").ok_or_else(invalid)?)?,
            },
            "scene-reconcile" => Command::SceneReconcile(SceneEdit::parse(args)?),
            "scene-save" => Command::SceneSave(SceneEdit::parse(args)?),
            "scene-reload" => Command::SceneReload(SceneEdit::parse(args)?),
            "tts-next" => Command::TtsNext,
            _ => return Err(invalid()),
        };
        Ok(Self { command, owner })
    }
    pub fn to_body(&self) -> String {
        let (name, mut args) = self.command.request();
        if let Some(owner) = &self.owner {
            args.as_object_mut()
                .unwrap()
                .insert("_owner".into(), json!(owner.as_str()));
        }
        json!({"command":name,"args":args}).to_string()
    }
}

impl Command {
    pub(crate) fn is_status_observation(&self) -> bool {
        matches!(self, Self::Status | Self::Diagnostics)
    }

    pub fn request(&self) -> (&'static str, Value) {
        match self {
            Self::Status => ("status", json!({})),
            Self::Heartbeat => ("heartbeat", json!({})),
            Self::Doctor => ("doctor", json!({})),
            Self::Devices { remote } => ("devices", json!({"remote":remote})),
            Self::Prepare { media } => ("prepare", json!({"media":media})),
            Self::Start(start) => {
                let mut args = json!({"output":start.output.as_str()});
                if let Some(seconds) = start.limit_seconds {
                    args["duration_seconds"] = json!(seconds);
                }
                if let Some(id) = &start.session_id {
                    args["session_id"] = json!(id);
                }
                if start.output == Output::TwitchLive {
                    args["confirm_public"] = json!(true);
                }
                ("start", args)
            }
            Self::Stop => ("stop", json!({})),
            Self::Routes { media } => ("routes", json!({"media":media})),
            Self::Mic(action) => (
                "mic",
                json!({"action":match action{MicAction::Status=>"status",MicAction::Mute=>"mute",MicAction::Unmute=>"unmute"}}),
            ),
            Self::TtsStatus => ("tts", json!({"action":"status"})),
            Self::Say { text, id } => ("say", json!({"text":text, "id":id})),
            Self::Diagnostics => ("diagnostics", json!({})),
            Self::Preview => ("preview", json!({})),
            Self::PreviewSmall => ("preview-small", json!({})),
            Self::Chat(cursor) => (
                "chat",
                cursor.as_ref().map_or_else(|| json!({}), ChatCursor::args),
            ),
            Self::Scene => ("scene", json!({})),
            Self::SceneApply { edit, layout } => {
                let mut args = edit.args();
                args["layout"] = layout.value();
                ("scene-apply", args)
            }
            Self::SceneReconcile(edit) => ("scene-reconcile", edit.args()),
            Self::SceneSave(edit) => ("scene-save", edit.args()),
            Self::SceneReload(edit) => ("scene-reload", edit.args()),
            Self::TtsNext => ("tts-next", json!({})),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_start_never_publishes() {
        assert_eq!(
            Invocation::parse("start", &json!({}), 200).unwrap().command,
            Command::Start(Start {
                output: Output::Recording,
                limit_seconds: None,
                session_id: None,
            })
        );
        assert!(Invocation::parse("start", &json!({"output":false}), 200).is_err());
    }
    #[test]
    fn rejects_unknown_args_and_ambiguous_body() {
        for body in [
            r#"{"command":"status","args":{"extra":true}}"#,
            r#"{"command":"status","args":null}"#,
            r#"{"command":"status","args":{},"token":"hidden"}"#,
            r#"{} {}"#,
        ] {
            assert!(Invocation::from_body(body, 200).is_err());
        }
    }
    #[test]
    fn text_roundtrips_without_losing_quotes_or_whitespace() {
        let original = Invocation {
            command: Command::Say {
                text: " \"quoted\"\n日本語 ".into(),
                id: "epoch-1".into(),
            },
            owner: None,
        };
        assert_eq!(
            Invocation::from_body(&original.to_body(), 200).unwrap(),
            original
        );
    }
}
