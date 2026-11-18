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
    Devices { remote: bool },
    Prepare { media: bool },
    Start(Start),
    Stop,
    Routes { media: bool },
    Mic(MicAction),
    TtsStatus,
    Say { text: String, id: String },
    Diagnostics,
    Preview,
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
            "status" | "heartbeat" | "doctor" | "stop" | "diagnostics" | "tts-next" | "preview" => {
                &[]
            }
            "devices" => &["remote"],
            "start" => &["output", "confirm_public", "duration_seconds", "session_id"],
            "prepare" | "routes" => &["media"],
            "mic" | "tts" => &["action"],
            "say" => &["text", "id"],
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
