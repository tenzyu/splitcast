use serde_json::{json, Map, Value};
use std::io;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Output {
    Recording,
}

impl Output {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Recording => "recording",
        }
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
    Start(Output),
    Stop,
    Routes { media: bool },
    Mic(MicAction),
    TtsStatus,
    Say { text: String, id: Option<String> },
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
            "start" => &["output"],
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
            "start" => Command::Start(
                match args
                    .get("output")
                    .map(|v| v.as_str().ok_or_else(invalid))
                    .transpose()?
                    .unwrap_or("recording")
                {
                    "recording" => Output::Recording,
                    _ => return Err(invalid()),
                },
            ),
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
                id: if args.contains_key("id") {
                    Some(id()?)
                } else {
                    None
                },
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
            Self::Start(output) => ("start", json!({"output":output.as_str()})),
            Self::Stop => ("stop", json!({})),
            Self::Routes { media } => ("routes", json!({"media":media})),
            Self::Mic(action) => (
                "mic",
                json!({"action":match action{MicAction::Status=>"status",MicAction::Mute=>"mute",MicAction::Unmute=>"unmute"}}),
            ),
            Self::TtsStatus => ("tts", json!({"action":"status"})),
            Self::Say { text, id } => ("say", {
                let mut v = json!({"text":text});
                if let Some(id) = id {
                    v["id"] = json!(id);
                }
                v
            }),
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
            Command::Start(Output::Recording)
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
                id: None,
            },
            owner: None,
        };
        assert_eq!(
            Invocation::from_body(&original.to_body(), 200).unwrap(),
            original
        );
    }
}
