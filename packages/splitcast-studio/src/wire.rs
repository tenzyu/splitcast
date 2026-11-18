use std::{io, path::Path, sync::Arc, time::Duration};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{codecs::png::PngDecoder, ImageDecoder as _, Limits, RgbaImage};
use serde::Deserialize;
use serde_json::{json, Value};
use splitcast_rs_core::config::secret;

#[derive(Clone)]
pub(crate) struct Endpoint {
    local_port: u16,
    agent_port: u16,
    token: Arc<str>,
}

impl Endpoint {
    pub(crate) fn new(local_port: u16, agent_port: u16, token_file: &Path) -> io::Result<Self> {
        let token = secret(token_file, 513)?;
        if !(24..=512).contains(&token.len())
            || !token.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
        {
            return Err(io::Error::other("invalid secret token"));
        }
        Ok(Self {
            local_port,
            agent_port,
            token: token.into(),
        })
    }

    pub(crate) fn request(&self, call: &Call) -> Result<Reply, Failure> {
        let agent = ureq::Agent::config_builder()
            .proxy(None)
            .max_redirects(0)
            .http_status_as_error(false)
            .timeout_global(Some(call.timeout()))
            .build()
            .new_agent();
        let origin = format!("http://127.0.0.1:{}", self.agent_port);
        let host = format!("127.0.0.1:{}", self.agent_port);
        let auth = format!("Bearer {}", self.token);
        let mut response = if let Call::Status = call {
            agent
                .get(format!("http://127.0.0.1:{}/api/status", self.local_port))
                .header("Host", host)
                .header("Origin", origin)
                .header("Authorization", auth)
                .header("Connection", "close")
                .call()
        } else {
            let (command, args) = call.command();
            let body = json!({"command": command, "args": args}).to_string();
            agent
                .post(format!("http://127.0.0.1:{}/api/command", self.local_port))
                .header("Host", host)
                .header("Origin", origin)
                .header("Authorization", auth)
                .header("Connection", "close")
                .header("Content-Type", "application/json")
                .send(body)
        }
        .map_err(|error| match error {
            ureq::Error::ConnectionFailed => Failure::Disconnected,
            _ => Failure::Unknown,
        })?;

        let code = response.status().as_u16();
        let cap = if matches!(call, Call::Preview) {
            768 * 1024 + 128
        } else {
            128 * 1024
        };
        let mut lengths = response.headers().get_all("content-length").iter();
        let length = lengths
            .next()
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|length| *length <= cap && lengths.next().is_none())
            .ok_or(Failure::Unknown)?;
        if response.headers().contains_key("transfer-encoding")
            || response.headers().contains_key("content-encoding")
        {
            return Err(Failure::Unknown);
        }
        let bytes = response
            .body_mut()
            .with_config()
            .limit(cap as u64)
            .read_to_vec()
            .map_err(|_| Failure::Unknown)?;
        if bytes.len() != length {
            return Err(Failure::Unknown);
        }

        if code == 403 {
            return Err(Failure::Unauthorized);
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| Failure::Unknown)?;
        if matches!(call, Call::Stop) {
            return match (code, value["stop"].as_str(), value["outcome"].as_str()) {
                (202, Some("requested"), Some("accepted")) => {
                    let id = value["stop_id"]
                        .as_str()
                        .filter(|id| valid_id(id))
                        .ok_or(Failure::Unknown)?;
                    Ok(Reply::Stop(StopReply::Admitted(id.to_owned())))
                }
                (200, Some("completed"), Some("completed")) => {
                    let status: Status = decode(value)?;
                    let id = status
                        .stop_id
                        .as_deref()
                        .filter(|id| valid_id(id))
                        .ok_or(Failure::Unknown)?;
                    if status.completed_stop(id) {
                        Ok(Reply::Stop(StopReply::Completed))
                    } else {
                        Err(Failure::Unknown)
                    }
                }
                (409, Some("incomplete"), Some("unknown")) => {
                    Ok(Reply::Stop(StopReply::Incomplete))
                }
                _ => Err(rejection(&value)),
            };
        }
        if code != 200 {
            return Err(rejection(&value));
        }
        match call {
            Call::Status => Ok(Reply::Status(decode(value)?)),
            Call::Scene | Call::SceneChange { .. } => {
                let scene: Scene = decode(value)?;
                if !scene.valid() {
                    return Err(Failure::Unknown);
                }
                Ok(Reply::Scene(Box::new(scene)))
            }
            Call::Chat { .. } => {
                let chat: ChatPage = decode(value)?;
                if !chat.valid() {
                    return Err(Failure::Unknown);
                }
                Ok(Reply::Chat(chat))
            }
            Call::Preview => decode_preview(&value).map(Reply::Preview),
            Call::Prepare { .. } | Call::Start(_) => Ok(Reply::Changed),
            Call::Stop => unreachable!("Stop replies are handled above"),
        }
    }
}

fn decode<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, Failure> {
    serde_json::from_value(value).map_err(|_| Failure::Unknown)
}

pub(crate) fn valid_id(id: &str) -> bool {
    (1..=128).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

fn rejection(value: &Value) -> Failure {
    if value["outcome"] != "not_executed" {
        return Failure::Unknown;
    }
    let code = value["code"].as_str();
    let text = value["error"].as_str().unwrap_or("");
    if matches!(
        code,
        Some("generation_conflict" | "stale_generation" | "ownership_conflict")
    ) || text.contains("generation")
        || text.contains("owner")
    {
        Failure::Conflict
    } else {
        Failure::Rejected
    }
}

fn decode_preview(value: &Value) -> Result<RgbaImage, Failure> {
    let payload = value["image"]
        .as_str()
        .and_then(|image| image.strip_prefix("data:image/png;base64,"))
        .filter(|payload| payload.len() <= 768 * 1024)
        .ok_or(Failure::Unknown)?;
    let png = STANDARD.decode(payload).map_err(|_| Failure::Unknown)?;
    let mut decoder = PngDecoder::new(io::Cursor::new(png)).map_err(|_| Failure::Unknown)?;
    let (width, height) = decoder.dimensions();
    if !(1..=480).contains(&width) || !(1..=270).contains(&height) {
        return Err(Failure::Unknown);
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(480);
    limits.max_image_height = Some(270);
    limits.max_alloc = Some(4 * 1024 * 1024);
    decoder.set_limits(limits).map_err(|_| Failure::Unknown)?;
    image::DynamicImage::from_decoder(decoder)
        .map(|image| image.into_rgba8())
        .map_err(|_| Failure::Unknown)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Failure {
    Disconnected,
    Unauthorized,
    Conflict,
    Rejected,
    Unknown,
    Superseded,
}

impl Failure {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Disconnected => "Disconnected",
            Self::Unauthorized => "Authentication refused",
            Self::Conflict => "Conflict · refresh the Agent scene",
            Self::Rejected => "Agent declined the request",
            Self::Unknown => "Outcome unknown · observe before retrying",
            Self::Superseded => "Superseded by Stop",
        }
    }
}

pub(crate) enum Call {
    Status,
    Stop,
    Prepare {
        media: bool,
    },
    Start(Value),
    Scene,
    SceneChange {
        action: SceneAction,
        generation: String,
        id: String,
        layout: Option<Layout>,
    },
    Chat {
        epoch: Option<String>,
        cursor: u64,
    },
    Preview,
}

impl Call {
    fn timeout(&self) -> Duration {
        Duration::from_secs(match self {
            Self::Status | Self::Chat { .. } => 2,
            Self::Stop | Self::Preview => 4,
            Self::Prepare { .. } | Self::Start(_) | Self::Scene | Self::SceneChange { .. } => 125,
        })
    }

    fn command(&self) -> (&'static str, Value) {
        match self {
            Self::Status => unreachable!("Status is a GET"),
            Self::Stop => ("stop", json!({})),
            Self::Prepare { media } => ("prepare", json!({"media": media})),
            Self::Start(args) => ("start", args.clone()),
            Self::Scene => ("scene", json!({})),
            Self::SceneChange {
                action,
                generation,
                id,
                layout,
            } => {
                let mut args = json!({"generation": generation, "operation": id});
                if let Some(layout) = layout {
                    args["layout"] = json!(layout);
                }
                (action.command(), args)
            }
            Self::Chat {
                epoch: Some(epoch),
                cursor,
            } => ("chat", json!({"epoch": epoch, "after": cursor})),
            Self::Chat { epoch: None, .. } => ("chat", json!({})),
            Self::Preview => ("preview-small", json!({})),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SceneAction {
    Apply,
    Reconcile,
    Save,
    Reload,
}

impl SceneAction {
    fn command(self) -> &'static str {
        match self {
            Self::Apply => "scene-apply",
            Self::Reconcile => "scene-reconcile",
            Self::Save => "scene-save",
            Self::Reload => "scene-reload",
        }
    }
}

pub(crate) enum Reply {
    Status(Status),
    Stop(StopReply),
    Changed,
    Scene(Box<Scene>),
    Chat(ChatPage),
    Preview(RgbaImage),
}

pub(crate) enum StopReply {
    Admitted(String),
    Completed,
    Incomplete,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub(crate) struct Status {
    pub role: Role,
    pub state: Phase,
    pub output: Option<String>,
    pub start_id: Option<String>,
    pub stop_id: Option<String>,
    pub stop: Option<String>,
    pub outcome: Option<String>,
    pub cleanup: Option<String>,
    pub remote_release_pending: bool,
}

impl Status {
    pub(crate) fn completed_stop(&self, id: &str) -> bool {
        self.stop_id.as_deref() == Some(id)
            && self.stop.as_deref() == Some("completed")
            && self.outcome.as_deref() == Some("completed")
            && self.state == Phase::Idle
            && !self.remote_release_pending
    }

    pub(crate) fn start_id(&self) -> Option<&str> {
        self.start_id
            .as_deref()
            .filter(|id| self.state == Phase::Ready && valid_id(id))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Role {
    Capture,
    Production,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Phase {
    Idle,
    Preparing,
    Ready,
    Degraded,
    Running,
    Stopping,
    Failed,
}

impl Phase {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Preparing => "Preparing",
            Self::Ready => "Ready",
            Self::Degraded => "Degraded",
            Self::Running => "Running",
            Self::Stopping => "Stopping",
            Self::Failed => "Failed",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Mode {
    Draft,
    Live,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Persistence {
    Saved,
    Unsaved,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum OperationState {
    Draft,
    Applied,
    Reconciled,
    Saved,
    Reloaded,
    Failed,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub(crate) struct Operation {
    pub id: String,
    pub state: OperationState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub(crate) struct Geometry {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub(crate) struct Sources {
    pub game: Geometry,
    pub camera: Option<Geometry>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, serde::Serialize)]
pub(crate) struct Camera {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub visible: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, serde::Serialize)]
pub(crate) struct Layout {
    pub camera: Option<Camera>,
    pub overlay: Overlay,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, serde::Serialize)]
pub(crate) struct Overlay {
    pub text: Option<String>,
    pub x: u32,
    pub y: u32,
}

impl Default for Overlay {
    fn default() -> Self {
        Self {
            text: None,
            x: 16,
            y: 16,
        }
    }
}

impl Layout {
    pub(crate) fn within(&self, canvas: &Geometry) -> bool {
        self.camera.as_ref().is_none_or(|camera| {
            camera.width > 0
                && camera.height > 0
                && camera
                    .x
                    .checked_add(camera.width)
                    .is_some_and(|edge| edge <= canvas.width)
                && camera
                    .y
                    .checked_add(camera.height)
                    .is_some_and(|edge| edge <= canvas.height)
        }) && self.overlay.x < canvas.width
            && self.overlay.y < canvas.height
            && self.overlay.text.as_ref().map_or(
                (self.overlay.x, self.overlay.y) == (16, 16),
                |text| {
                    !text.trim().is_empty()
                        && text.chars().count() <= 80
                        && !text.chars().any(char::is_control)
                },
            )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub(crate) struct Scene {
    pub generation: String,
    pub operation: Option<Operation>,
    pub canvas: Geometry,
    pub sources: Option<Sources>,
    pub mode: Mode,
    pub draft: Layout,
    pub applied: Option<Layout>,
    pub saved: Option<Layout>,
    pub persistence: Persistence,
}

impl Scene {
    fn valid(&self) -> bool {
        valid_id(&self.generation)
            && self.canvas.width <= 16384
            && self.canvas.height <= 16384
            && self.draft.within(&self.canvas)
            && self
                .applied
                .as_ref()
                .is_none_or(|layout| layout.within(&self.canvas))
            && self
                .saved
                .as_ref()
                .is_none_or(|layout| layout.within(&self.canvas))
            && self
                .operation
                .as_ref()
                .is_none_or(|operation| valid_id(&operation.id))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub(crate) struct Message {
    pub cursor: u64,
    pub id: String,
    pub display_name: String,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub(crate) struct Subscription {
    pub state: String,
    pub notifications: u64,
    pub duplicates: u64,
    pub dropped: u64,
    pub reconnects: u64,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChatPage {
    pub enabled: bool,
    pub epoch: Option<String>,
    pub cursor: u64,
    pub more: bool,
    pub reset: bool,
    pub overrun: bool,
    pub subscription: Option<Subscription>,
    pub messages: Vec<Message>,
}

impl ChatPage {
    fn valid(&self) -> bool {
        self.cursor <= 9_007_199_254_740_991
            && self.epoch.as_ref().is_none_or(|epoch| valid_id(epoch))
            && self.messages.len() <= 32
            && self
                .subscription
                .as_ref()
                .is_none_or(|sub| sub.state.len() <= 64)
            && self
                .messages
                .windows(2)
                .all(|items| items[0].cursor < items[1].cursor)
            && self.messages.iter().all(|item| {
                item.cursor > 0
                    && item.cursor <= self.cursor
                    && item.id.len() <= 128
                    && item.id.is_ascii()
                    && plain(&item.display_name, 128)
                    && plain(&item.text, 512)
            })
    }
}

fn plain(text: &str, limit: usize) -> bool {
    text.len() <= limit && !text.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageEncoder;

    #[test]
    fn bounds_png_before_allocating_pixels() {
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(&vec![0; 481 * 4], 481, 1, image::ExtendedColorType::Rgba8)
            .unwrap();
        let value = json!({"image": format!("data:image/png;base64,{}", STANDARD.encode(bytes))});
        assert_eq!(decode_preview(&value), Err(Failure::Unknown));
    }
}
