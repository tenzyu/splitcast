use serde_json::Value;
use std::fs::OpenOptions;
use std::io::{self, Read};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

fn error(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg)
}

fn field<'a>(value: &'a Value, key: &str) -> io::Result<&'a Value> {
    key.trim_start_matches('.')
        .split('.')
        .try_fold(value, |v, k| {
            v.get(k)
                .ok_or_else(|| error("required configuration field missing"))
        })
}
fn get(value: &Value, key: &str) -> io::Result<String> {
    let s = field(value, key)?
        .as_str()
        .ok_or_else(|| error("configuration string required"))?;
    if s.contains(['\0', '\n', '\r']) {
        return Err(error("invalid config string"));
    }
    Ok(s.into())
}
fn number(value: &Value, key: &str, min: u16, max: u16) -> io::Result<u16> {
    let n = field(value, key)?
        .as_u64()
        .and_then(|n| u16::try_from(n).ok())
        .filter(|n| (*n >= min) && (*n <= max))
        .ok_or_else(|| error("configuration integer out of range"))?;
    Ok(n)
}
fn signed_number(value: &Value, key: &str, min: i32, max: i32) -> io::Result<i32> {
    field(value, key)?
        .as_i64()
        .and_then(|n| i32::try_from(n).ok())
        .filter(|n| *n >= min && *n <= max)
        .ok_or_else(|| error("configuration integer out of range"))
}
fn boolean(value: &Value, key: &str) -> io::Result<bool> {
    field(value, key)?
        .as_bool()
        .ok_or_else(|| error("configuration boolean required"))
}

fn valid_schema(value: &Value, schema: &Value) -> bool {
    let valid = match schema["type"].as_str() {
        Some("object") => match (value.as_object(), schema["properties"].as_object()) {
            (Some(v), Some(p)) => {
                schema["required"].as_array().is_none_or(|required| {
                    required
                        .iter()
                        .all(|k| k.as_str().is_some_and(|k| v.contains_key(k)))
                }) && v.iter().all(|(k, value)| {
                    p.get(k)
                        .is_some_and(|property| valid_schema(value, property))
                })
            }
            _ => false,
        },
        Some("array") => value.as_array().is_some_and(|a| {
            a.len() >= schema["minItems"].as_u64().unwrap_or(0) as usize
                && a.len() <= schema["maxItems"].as_u64().unwrap_or(100) as usize
                && a.iter().all(|v| valid_schema(v, &schema["items"]))
        }),
        Some("string") => value.as_str().is_some_and(|s| {
            s.chars().count() <= schema["maxLength"].as_u64().unwrap_or(4096) as usize
                && !s.contains(['\0', '\n', '\r'])
                && schema
                    .get("pattern")
                    .and_then(Value::as_str)
                    .is_none_or(|p| {
                        regex::Regex::new(&format!("\\A(?:{p})\\z")).is_ok_and(|r| r.is_match(s))
                    })
        }),
        Some("integer") => value.as_i64().is_some_and(|n| {
            schema["minimum"].as_i64().is_none_or(|min| n >= min)
                && schema["maximum"].as_i64().is_none_or(|max| n <= max)
        }),
        Some("boolean") => value.is_boolean(),
        _ => true,
    };
    valid
        && schema.get("const").is_none_or(|v| v == value)
        && schema
            .get("enum")
            .and_then(Value::as_array)
            .is_none_or(|a| a.contains(value))
}

fn valid_connections(c: &Value) -> bool {
    let Some(url) = c["tts"]["voicevox_url"].as_str() else {
        return false;
    };
    let Some(origin) = url.strip_prefix("http://127.0.0.1") else {
        return false;
    };
    let origin = origin.strip_suffix('/').unwrap_or(origin);
    if !origin.is_empty()
        && !origin.strip_prefix(':').is_some_and(|p| {
            !p.is_empty()
                && p.bytes().all(|b| b.is_ascii_digit())
                && p.parse::<u16>().is_ok_and(|n| n > 0)
        })
    {
        return false;
    }
    let ports = [
        &c["peer"]["control_port"],
        &c["peer"]["forward_control_port"],
        &c["obs"]["websocket_port"],
        &c["capture"]["rtmp_port"],
        &c["capture"]["rtsp_port"],
        &c["capture"]["api_port"],
    ];
    if ports
        .iter()
        .enumerate()
        .any(|(i, p)| ports[..i].contains(p))
    {
        return false;
    }
    let Some(audio) = c["capture"]["game_audio"].as_str() else {
        return false;
    };
    if !audio.is_empty()
        && (!audio
            .strip_prefix("app:")
            .is_some_and(|s| !s.is_empty() && !s.contains(['|', ';', '\n', '\r', '\0']))
            || audio.eq_ignore_ascii_case("app:splitcast-tts"))
    {
        return false;
    }
    true
}
fn safe_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 256
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
}
pub fn is_raw_microphone(name: &str) -> bool {
    safe_name(name)
        && !["monitor", "easyeffects", "rnnoise", "splitcast"]
            .iter()
            .any(|part| name.to_ascii_lowercase().contains(part))
}
fn path(s: String) -> PathBuf {
    if s == "~" {
        return dirs_home();
    }
    if let Some(rest) = s.strip_prefix("~/") {
        return dirs_home().join(rest);
    }
    PathBuf::from(s)
}
fn dirs_home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/nonexistent"))
}

fn toml_json_compatible(value: &toml::Value) -> bool {
    match value {
        toml::Value::Datetime(_) => false,
        toml::Value::Float(n) => n.is_finite(),
        toml::Value::Array(items) => items.iter().all(toml_json_compatible),
        toml::Value::Table(items) => items.values().all(toml_json_compatible),
        _ => true,
    }
}

pub fn secret(path: &Path, max_bytes: usize) -> io::Result<String> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(0x800)
        .open(path)
        .map_err(|_| error("credential unavailable"))?;
    let m = file
        .metadata()
        .map_err(|_| error("credential unavailable"))?;
    unsafe extern "C" {
        fn getuid() -> u32;
    }
    if !m.is_file() || m.mode() & 0o077 != 0 || m.uid() != unsafe { getuid() } {
        return Err(error(
            "credential must be a private file owned by this user",
        ));
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(max_bytes.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| error("credential unavailable"))?;
    if bytes.len() > max_bytes {
        return Err(error("credential oversized"));
    }
    let value = std::str::from_utf8(&bytes)
        .map_err(|_| error("invalid credential"))?
        .trim();
    if value.is_empty() {
        return Err(error("credential empty"));
    }
    Ok(value.to_owned())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObsVideoMode {
    Hardware,
    Software,
}

#[derive(Clone, Debug)]
pub(crate) struct Twitch {
    pub(crate) ingest_host: String,
    pub(crate) key_file: PathBuf,
    pub(crate) allow_public: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct Camera {
    pub(crate) location: String,
    pub(crate) device: PathBuf,
    pub(crate) input_format: String,
    pub(crate) encoder: String,
    pub(crate) render_device: Option<PathBuf>,
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) fps: u16,
}

/// Configuration accepted by the SplitCast schema and role checks.
///
/// ```compile_fail
/// use splitcast_rs_core::config::Config;
/// fn bypass_port_validation(config: &mut Config) {
///     config.control_port = 0;
/// }
/// ```
#[derive(Clone, Debug)]
pub struct Config {
    #[cfg(test)]
    document: String,
    pub(crate) role: String,
    pub(crate) state_dir: PathBuf,
    pub(crate) token_file: PathBuf,
    pub(crate) remote_token_file: PathBuf,
    pub(crate) ssh_host: String,
    pub(crate) ssh_config: Option<PathBuf>,
    pub(crate) ssh_port: u16,
    pub(crate) control_port: u16,
    pub(crate) remote_control_port: u16,
    pub(crate) forward_control_port: u16,
    pub(crate) timeout: u16,
    pub(crate) rtmp: u16,
    pub(crate) rtsp: u16,
    pub(crate) api: u16,
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) fps: u16,
    pub(crate) bitrate: u16,
    pub(crate) target: String,
    pub(crate) game_audio: String,
    pub(crate) mic_source: String,
    pub(crate) tts_sink: String,
    pub(crate) tts_volume_percent: u16,
    pub(crate) processed_sink: String,
    pub(crate) rnnoise_plugin: PathBuf,
    pub(crate) rnnoise_label: String,
    pub(crate) rnnoise_control: String,
    pub(crate) gain_db: i32,
    pub(crate) mic_offset: i32,
    pub(crate) obs_binary: PathBuf,
    pub(crate) obs_video_mode: ObsVideoMode,
    pub(crate) obs_port: u16,
    pub(crate) obs_password: PathBuf,
    pub(crate) obs_bitrate: u16,
    pub(crate) separate_audio_tracks: bool,
    pub(crate) default_scene: crate::scene::SceneLayout,
    pub(crate) recording_dir: PathBuf,
    pub(crate) twitch: Option<Twitch>,
    pub(crate) camera: Option<Camera>,
    pub(crate) tts_enabled: bool,
    pub(crate) voicevox_url: String,
    pub(crate) tts_speaker: u16,
    pub(crate) tts_max_text: u16,
    pub(crate) tts_queue_limit: u16,
    pub(crate) tts_expiry_seconds: u16,
    pub(crate) chat_credentials: Option<PathBuf>,
}
impl Config {
    pub fn role(&self) -> &str {
        &self.role
    }
    pub fn control_port(&self) -> u16 {
        self.control_port
    }
    pub fn remote_control_port(&self) -> u16 {
        self.remote_control_port
    }
    pub fn ssh_host(&self) -> &str {
        &self.ssh_host
    }
    pub fn max_speech_text(&self) -> usize {
        self.tts_max_text.into()
    }

    pub fn load(filename: &Path) -> io::Result<Self> {
        let mut input = Vec::new();
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(0x800)
            .open(filename)?;
        if !file.metadata()?.is_file() {
            return Err(error("config must be a regular file"));
        }
        file.take(65537).read_to_end(&mut input)?;
        if input.len() > 65536 {
            return Err(error("config exceeds 64 KiB"));
        }
        if filename.extension().is_some_and(|ext| ext == "toml") {
            let text = std::str::from_utf8(&input).map_err(|_| error("invalid TOML UTF-8"))?;
            let value: toml::Value = text.parse().map_err(|_| error("invalid TOML"))?;
            if !toml_json_compatible(&value) {
                return Err(error("TOML contains unsupported value"));
            }
            let json = serde_json::to_vec(&value).map_err(|_| error("invalid TOML value"))?;
            Self::parse(&json)
        } else {
            Self::parse(&input)
        }
    }
    pub fn parse(filename: &[u8]) -> io::Result<Self> {
        if filename.len() > 65536 {
            return Err(error("config exceeds 64 KiB"));
        }
        let value: serde_json::Value =
            serde_json::from_slice(filename).map_err(|_| error("invalid JSON"))?;
        #[cfg(test)]
        let document = serde_json::to_string(&value).map_err(|_| error("invalid JSON"))?;
        let schema: Value = serde_json::from_str(include_str!("config-schema.json"))
            .map_err(|_| error("invalid built-in schema"))?;
        if !valid_schema(&value, &schema) || !valid_connections(&value) {
            return Err(error("unsupported SplitCast config"));
        }
        let filename = &value;
        let role = get(filename, ".role")?;
        let overlay_text = filename["obs"]["overlay_text"]
            .as_str()
            .map(|text| {
                if role != "production"
                    || text.trim().is_empty()
                    || text.chars().count() > 80
                    || text.chars().any(char::is_control)
                {
                    return Err(error(
                        "OBS overlay text must be a nonempty single line on production",
                    ));
                }
                Ok(text.to_owned())
            })
            .transpose()?;
        let ssh_host = get(filename, ".peer.ssh_host")?;
        if ssh_host.len() > 253
            || !ssh_host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._@-".contains(&b))
            || ssh_host.starts_with('-')
        {
            return Err(error("invalid ssh_host"));
        }
        let target = get(filename, ".capture.target")?;
        let game_audio = get(filename, ".capture.game_audio")?;
        let mic_source = get(filename, ".audio.mic_source")?;
        if !mic_source.is_empty() && !is_raw_microphone(&mic_source) {
            return Err(error(
                "mic_source must select a raw microphone, not a monitor or processed source",
            ));
        }
        let processed_sink = get(filename, ".audio.processed_sink")?;
        if !processed_sink.starts_with("splitcast_") || !safe_name(&processed_sink) {
            return Err(error("unsafe processed sink"));
        }
        let rnnoise_plugin = path(get(filename, ".audio.rnnoise_plugin")?);
        if !rnnoise_plugin.as_os_str().is_empty()
            && (!rnnoise_plugin.is_absolute()
                || !rnnoise_plugin
                    .to_string_lossy()
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"/._+-".contains(&b)))
        {
            return Err(error("unsafe RNNoise LADSPA path"));
        }
        let rnnoise_label = get(filename, ".audio.rnnoise_label")?;
        let rnnoise_control = get(filename, ".audio.rnnoise_control")?;
        if !safe_name(&rnnoise_label)
            || !rnnoise_control
                .bytes()
                .all(|b| b.is_ascii_digit() || b".|-".contains(&b))
        {
            return Err(error("unsafe RNNoise filter"));
        }
        let gain_db = signed_number(filename, ".audio.gain_db", -30, 20)?;
        let mic_offset = signed_number(filename, ".audio.mic_offset_ms", -2000, 2000)?;
        let legacy_chat = boolean(filename, ".tts.eventsub")?;
        let chat_credentials = match (legacy_chat, filename.get("chat")) {
            (true, Some(_)) => return Err(error("configure one EventSub credential source")),
            (true, None) => Some(path(get(filename, ".tts.credentials_file")?)),
            (false, Some(_)) => Some(path(get(filename, ".chat.credentials_file")?)),
            (false, None) => None,
        };
        if chat_credentials.is_some() && role != "production" {
            return Err(error("EventSub requires production"));
        }
        let twitch = if filename.get("twitch").is_some() {
            if role != "production" {
                return Err(error("Twitch broadcast settings belong to production"));
            }
            let key_file = path(get(filename, ".twitch.key_file")?);
            if !key_file.is_absolute() {
                return Err(error("Twitch key file must have an absolute path"));
            }
            Some(Twitch {
                ingest_host: get(filename, ".twitch.ingest_host")?,
                key_file,
                allow_public: filename["twitch"]["allow_public"] == true,
            })
        } else {
            None
        };
        let camera = if filename.get("camera").is_some() {
            let location = get(filename, ".camera.location")?;
            let device = filename["camera"]["device"].as_str().unwrap_or("");
            let input_format = get(filename, ".camera.input_format")?;
            let encoder = get(filename, ".camera.encoder")?;
            let render_device = filename["camera"]["render_device"]
                .as_str()
                .map(PathBuf::from);
            if role == location && device.is_empty() {
                return Err(error("the camera host requires an explicit stable device"));
            }
            if (encoder == "copy") != (input_format == "h264")
                || (role == location && encoder == "h264_vaapi" && render_device.is_none())
            {
                return Err(error("camera format and encoder do not match"));
            }
            Some(Camera {
                location,
                device: device.into(),
                input_format,
                encoder,
                render_device,
                width: number(filename, ".camera.width", 160, 3840)?,
                height: number(filename, ".camera.height", 120, 2160)?,
                fps: number(filename, ".camera.fps", 1, 60)?,
            })
        } else {
            None
        };
        let width = number(filename, ".capture.width", 320, 3840)?;
        let height = number(filename, ".capture.height", 240, 2160)?;
        let default_scene =
            crate::scene::SceneLayout::default_for(width, height, camera.is_some(), overlay_text);
        Ok(Self {
            #[cfg(test)]
            document,
            role,
            ssh_host,
            ssh_config: filename["peer"]["ssh_config"]
                .as_str()
                .map(|value| path(value.into())),
            state_dir: path(get(filename, ".peer.state_dir")?),
            token_file: path(get(filename, ".peer.token_file")?),
            remote_token_file: path(get(filename, ".peer.remote_token_file")?),
            ssh_port: number(filename, ".peer.ssh_port", 1, 65535)?,
            control_port: number(filename, ".peer.control_port", 1024, 65535)?,
            remote_control_port: number(filename, ".peer.remote_control_port", 1024, 65535)?,
            forward_control_port: number(filename, ".peer.forward_control_port", 1024, 65535)?,
            timeout: number(filename, ".peer.timeout_seconds", 1, 60)?,
            rtmp: number(filename, ".capture.rtmp_port", 1024, 65535)?,
            rtsp: number(filename, ".capture.rtsp_port", 1024, 65535)?,
            api: number(filename, ".capture.api_port", 1024, 65535)?,
            width,
            height,
            fps: number(filename, ".capture.fps", 30, 60)?,
            bitrate: number(filename, ".capture.bitrate_kbps", 1000, 50000)?,
            target,
            game_audio,
            mic_source,
            tts_sink: get(filename, ".audio.tts_sink")?,
            tts_volume_percent: filename["audio"]["tts_volume_percent"]
                .as_u64()
                .map_or(100, |volume| volume as u16),
            processed_sink,
            rnnoise_plugin,
            rnnoise_label,
            rnnoise_control,
            gain_db,
            mic_offset,
            obs_binary: path(get(filename, ".obs.binary")?),
            obs_video_mode: match filename["obs"].get("video_mode").and_then(Value::as_str) {
                None | Some("hardware") => ObsVideoMode::Hardware,
                Some("software") => ObsVideoMode::Software,
                Some(_) => return Err(error("invalid OBS video mode")),
            },
            obs_port: number(filename, ".obs.websocket_port", 1024, 65535)?,
            obs_password: path(get(filename, ".obs.password_file")?),
            obs_bitrate: number(filename, ".obs.bitrate_kbps", 1000, 10000)?,
            separate_audio_tracks: filename["obs"]["separate_audio_tracks"] == true,
            default_scene,
            recording_dir: path(get(filename, ".obs.recording_dir")?),
            twitch,
            camera,
            tts_enabled: boolean(filename, ".tts.enabled")?,
            voicevox_url: get(filename, ".tts.voicevox_url")?,
            tts_speaker: number(filename, ".tts.speaker", 0, 10000)?,
            tts_max_text: number(filename, ".tts.max_text", 1, 500)?,
            tts_queue_limit: number(filename, ".tts.queue_limit", 1, 32)?,
            tts_expiry_seconds: number(filename, ".tts.expiry_seconds", 5, 300)?,
            chat_credentials,
        })
    }
    pub fn token(&self, remote: bool) -> io::Result<String> {
        let p = if remote {
            &self.remote_token_file
        } else {
            &self.token_file
        };
        let s = secret(p, 513)?;
        if s.len() < 24 || s.len() > 512 || !s.bytes().all(|b| (0x21..=0x7e).contains(&b)) {
            return Err(error("invalid secret token"));
        }
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn studio_chat_config_accepts_one_production_credential_source_with_speech_off() {
        let mut input: Value =
            serde_json::from_str(include_str!("../tests/agent-example.json")).unwrap();
        let parse = |input: &Value| Config::parse(input.to_string().as_bytes());
        input["chat"] = serde_json::json!({"credentials_file":"/private/chat.json"});
        assert!(parse(&input).is_err());
        input["role"] = "production".into();
        let independent = parse(&input).unwrap();
        assert!(!independent.tts_enabled);
        assert_eq!(
            independent.chat_credentials.unwrap(),
            Path::new("/private/chat.json")
        );
        input["tts"]["eventsub"] = true.into();
        assert!(parse(&input).is_err());
        input.as_object_mut().unwrap().remove("chat");
        let legacy_without_speech = parse(&input).unwrap();
        assert!(legacy_without_speech.chat_credentials.is_some());
        input["tts"]["enabled"] = true.into();
        assert!(parse(&input).unwrap().chat_credentials.is_some());
    }

    #[test]
    fn camera_requires_a_stable_device_only_on_its_selected_role() {
        let mut config: Value =
            serde_json::from_str(include_str!("../tests/example.json")).unwrap();
        config["camera"] = serde_json::json!({
            "location":"capture","input_format":"mjpeg","encoder":"libx264",
            "width":320,"height":180,"fps":15
        });
        let parse = |value: &Value| Config::parse(value.to_string().as_bytes());
        assert!(parse(&config).is_err());
        config["camera"]["device"] = "/dev/video0".into();
        assert!(parse(&config).is_err());
        config["camera"]["device"] = "/dev/v4l/by-id/usb-example-video-index0".into();
        assert!(parse(&config).is_ok());
        config["role"] = "production".into();
        config["camera"].as_object_mut().unwrap().remove("device");
        assert!(parse(&config).is_ok());
        config["camera"]["encoder"] = "copy".into();
        assert!(parse(&config).is_err());
    }

    #[test]
    fn explicit_software_video_mode_is_accepted() {
        let mut input: Value = serde_json::from_str(include_str!("../tests/example.json")).unwrap();
        let legacy = Config::parse(input.to_string().as_bytes()).unwrap();
        assert_eq!(legacy.obs_video_mode, ObsVideoMode::Hardware);
        assert_eq!(
            serde_json::from_str::<Value>(&legacy.document).unwrap(),
            input
        );
        input["obs"]["video_mode"] = Value::String("hardware".into());
        assert_eq!(
            Config::parse(input.to_string().as_bytes())
                .unwrap()
                .obs_video_mode,
            ObsVideoMode::Hardware
        );
        input["obs"]["video_mode"] = Value::String("software".into());
        assert_eq!(
            Config::parse(input.to_string().as_bytes())
                .unwrap()
                .obs_video_mode,
            ObsVideoMode::Software
        );
        for invalid in [
            serde_json::json!(null),
            serde_json::json!("automatic"),
            serde_json::json!(false),
        ] {
            input["obs"]["video_mode"] = invalid;
            assert!(Config::parse(input.to_string().as_bytes()).is_err());
        }
        input["obs"]["video_mode"] = serde_json::json!("software");
        input["obs"]["unexpected"] = serde_json::json!(true);
        assert!(Config::parse(input.to_string().as_bytes()).is_err());
    }

    #[test]
    fn every_declared_required_field_remains_mandatory() {
        fn check(value: &Value, schema: &Value) {
            if let Some(properties) = schema["properties"].as_object() {
                for key in schema["required"].as_array().unwrap() {
                    let key = key.as_str().unwrap();
                    let mut missing = value.clone();
                    missing.as_object_mut().unwrap().remove(key);
                    assert!(!valid_schema(&missing, schema), "required field {key}");
                }
                for (key, property) in properties {
                    if let Some(child) = value.get(key) {
                        check(child, property);
                    }
                }
            }
            if let Some(items) = value.as_array() {
                for item in items {
                    check(item, &schema["items"]);
                }
            }
        }
        let input: Value = serde_json::from_str(include_str!("../tests/example.json")).unwrap();
        let schema: Value = serde_json::from_str(include_str!("config-schema.json")).unwrap();
        check(&input, &schema);
    }

    #[test]
    fn toml_example_loads() {
        let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("example.toml");
        let config = Config::load(&file).expect("valid TOML must load");
        let json = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/example.json"))
            .unwrap();
        let expected: serde_json::Value = serde_json::from_slice(&json).unwrap();
        let actual: serde_json::Value = serde_json::from_str(&config.document).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn toml_rejects_invalid_schema_and_non_json_values() {
        let example =
            std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("example.toml"))
                .unwrap();
        let file =
            std::env::temp_dir().join(format!("splitcast-config-{}.toml", std::process::id()));
        for (name, input) in [
            (
                "unknown field",
                example.replace("ssh_host =", "unknown = 1\nssh_host ="),
            ),
            (
                "wrong type",
                example.replacen("width = 1280", "width = true", 1),
            ),
            (
                "non-finite",
                example.replacen("width = 1280", "width = inf", 1),
            ),
            (
                "datetime",
                example.replace("ssh_host = \"production-host\"", "ssh_host = 2026-01-01"),
            ),
        ] {
            std::fs::write(&file, input).unwrap();
            assert!(Config::load(&file).is_err(), "{name} must be rejected");
        }
        std::fs::remove_file(file).unwrap();
    }

    #[test]
    fn json_escaping() {}

    #[test]
    fn preserves_strings_and_false() {
        let input = br#"{"text":"a b ","flag":false,"bad":"a\nb"}"#;
        let input: Value = serde_json::from_slice(input).unwrap();
        assert_eq!(get(&input, ".text").unwrap(), "a b ");
        assert!(!boolean(&input, ".flag").unwrap());
        assert!(get(&input, ".bad").is_err());
    }

    #[test]
    fn oversized_config_is_rejected() {
        let name = std::env::temp_dir().join(format!("splitcast-config-{}", std::process::id()));
        std::fs::write(&name, vec![b' '; 65537]).unwrap();
        let result = Config::load(&name);
        std::fs::remove_file(name).unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn secret_requires_private_regular_bounded_file() {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        let name =
            std::env::temp_dir().join(format!("splitcast-secret-test-{}", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true).mode(0o600);
        let mut file = options.open(&name).unwrap();
        file.write_all(b"test credential\n").unwrap();
        assert_eq!(secret(&name, 16).unwrap(), "test credential");
        assert!(secret(&name, 4).is_err());
        std::fs::set_permissions(&name, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(secret(&name, 32).is_err());
        std::fs::remove_file(name).unwrap();
        assert!(secret(std::env::temp_dir().as_path(), 32).is_err());
    }
}
