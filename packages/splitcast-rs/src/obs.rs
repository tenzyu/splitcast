//! Authenticated OBS WebSocket v5 control over loopback TCP only.
use crate::config::{secret, Config, ObsVideoMode};
use crate::scene::{CameraBox, Observation, Overlay, SceneLayout, Size, OVERLAY_ORIGIN};
use crate::state::{atomic_file, private_directory};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tungstenite::{protocol::WebSocketConfig, Message, WebSocket};

const LIMIT: usize = 4 * 1024 * 1024;
pub(crate) const TIMEOUT: Duration = Duration::from_secs(5);

fn err(message: &str) -> io::Error {
    io::Error::other(message)
}
fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "OBS control deadline exceeded"))
}

struct DeadlineStream {
    stream: TcpStream,
    deadline: Instant,
    operation: Option<crate::request::Operation>,
}
impl Read for DeadlineStream {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if let Some(operation) = &self.operation {
            return operation.read(&mut self.stream, bytes, self.deadline);
        }
        self.stream
            .set_read_timeout(Some(remaining(self.deadline)?))?;
        self.stream.read(bytes).map_err(normalize_timeout)
    }
}
impl Write for DeadlineStream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(operation) = &self.operation {
            return operation.write(&mut self.stream, bytes, self.deadline);
        }
        self.stream
            .set_write_timeout(Some(remaining(self.deadline)?))?;
        self.stream.write(bytes).map_err(normalize_timeout)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stream
            .set_write_timeout(Some(remaining(self.deadline)?))?;
        self.stream.flush().map_err(normalize_timeout)
    }
}
fn normalize_timeout(e: io::Error) -> io::Error {
    if matches!(
        e.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    ) {
        io::Error::new(io::ErrorKind::TimedOut, "OBS control deadline exceeded")
    } else {
        e
    }
}
fn redacted_websocket_error(e: tungstenite::Error) -> io::Error {
    match e {
        tungstenite::Error::Io(e) => normalize_timeout(e),
        tungstenite::Error::Capacity(_) => err("oversized OBS response"),
        _ => err("invalid or closed OBS WebSocket"),
    }
}
fn base64_sha(input: &[u8]) -> String {
    STANDARD.encode(Sha256::digest(input))
}
fn auth_string<'a>(hello: &'a Value, key: &str) -> io::Result<&'a str> {
    hello["d"]["authentication"][key]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 4096)
        .ok_or_else(|| err("OBS authentication string required"))
}
fn response_data(response: &str) -> io::Result<Value> {
    let value: Value = serde_json::from_str(response).map_err(|_| err("invalid OBS JSON"))?;
    value["d"]["responseData"]
        .as_object()
        .map(|_| value["d"]["responseData"].clone())
        .ok_or_else(|| err("OBS response data required"))
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct OutputStatus {
    active: bool,
    bytes: u64,
    frames: Option<u64>,
    duration_ms: Option<u64>,
}

impl OutputStatus {
    fn from_response(output: crate::Output, response: &Value) -> io::Result<Self> {
        let frames = match output {
            // OBS 32.1.2's GetRecordStatus does not report outputTotalFrames.
            crate::Output::Recording => None,
            crate::Output::TwitchTest | crate::Output::TwitchLive => Some(
                response["outputTotalFrames"]
                    .as_u64()
                    .ok_or_else(|| err("OBS stream outputTotalFrames unsigned counter required"))?,
            ),
        };
        Ok(Self {
            active: response["outputActive"]
                .as_bool()
                .ok_or_else(|| err("OBS outputActive boolean required"))?,
            bytes: response["outputBytes"]
                .as_u64()
                .ok_or_else(|| err("OBS outputBytes unsigned counter required"))?,
            frames,
            duration_ms: response["outputDuration"].as_u64(),
        })
    }

    fn progresses_from(&self, previous: Option<&Self>) -> bool {
        self.active
            && self.bytes > previous.map_or(0, |sample| sample.bytes)
            && match (self.frames, previous.and_then(|sample| sample.frames)) {
                (None, None) => true,
                (Some(now), None) if previous.is_none() => now > 0,
                (Some(now), Some(old)) => now > old,
                _ => false,
            }
    }

    pub(crate) fn ready(&self) -> bool {
        self.progresses_from(None)
    }
}

pub(crate) fn read_output_status(
    port: u16,
    password_path: &Path,
    output: crate::Output,
    operation: Option<&crate::request::Operation>,
) -> io::Result<OutputStatus> {
    let operation = operation
        .cloned()
        .map(|operation| operation.capped(TIMEOUT));
    Obs::connect_endpoint(port, password_path, operation)?.observe(output)
}

pub(crate) struct OutputProgress(ProgressSnapshot);

#[derive(Clone, Copy, Debug)]
pub(crate) struct ProgressSnapshot {
    sample: OutputStatus,
    compared: bool,
}

impl OutputProgress {
    pub(crate) fn start(sample: OutputStatus) -> Option<Self> {
        sample.ready().then_some(Self(ProgressSnapshot {
            sample,
            compared: false,
        }))
    }

    pub(crate) fn snapshot(&self) -> ProgressSnapshot {
        self.0
    }

    pub(crate) fn observe(&mut self, sample: OutputStatus) -> Option<ProgressSnapshot> {
        if !sample.progresses_from(Some(&self.0.sample)) {
            return None;
        }
        self.0 = ProgressSnapshot {
            sample,
            compared: true,
        };
        Some(self.0)
    }
}

impl ProgressSnapshot {
    pub(crate) fn status(self) -> Value {
        let Self { sample, compared } = self;
        let frame_health = match (sample.frames, compared) {
            (None, _) => "unknown",
            (Some(_), false) => "pending",
            (Some(_), true) => "progressing",
        };
        json!({
            "active": sample.active, "bytes": sample.bytes, "frames": sample.frames,
            "frame_health": frame_health, "duration_ms": sample.duration_ms,
        })
    }
}

#[cfg(test)]
impl ProgressSnapshot {
    pub(crate) fn test_recording(bytes: u64) -> Self {
        OutputProgress::start(OutputStatus {
            active: true,
            bytes,
            frames: None,
            duration_ms: None,
        })
        .unwrap()
        .snapshot()
    }
}

// The profile retains the attempted output independently of progress. A lost
// Start reply can leave output active before any counters are observed.
pub(crate) struct SessionProfile {
    directory: PathBuf,
    attempt: Option<crate::Output>,
}

impl SessionProfile {
    pub(crate) fn create(directory: PathBuf) -> io::Result<Self> {
        fs::create_dir(&directory)?;
        Ok(Self {
            directory,
            attempt: None,
        })
    }

    #[cfg(test)]
    pub(crate) fn test_attempt(directory: PathBuf, output: crate::Output) -> Self {
        Self {
            directory,
            attempt: Some(output),
        }
    }

    pub(crate) fn launch_args(&self, cfg: &Config) -> io::Result<Vec<String>> {
        launch_args(cfg, &self.directory)
    }

    pub(crate) fn request_start(&mut self, obs: &mut Obs, output: crate::Output) -> io::Result<()> {
        self.attempt = Some(output);
        obs.start(output)
    }

    pub(crate) fn requested_output(&self) -> Option<crate::Output> {
        self.attempt
    }

    pub(crate) fn discard(&self) -> io::Result<()> {
        match fs::remove_dir_all(&self.directory) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }
}
pub struct Obs {
    operation: Option<crate::request::Operation>,
    socket: WebSocket<DeadlineStream>,
    seq: u64,
    failed: bool,
}
impl Obs {
    #[cfg(test)]
    fn connect(cfg: &Config) -> io::Result<Self> {
        Self::connect_until(cfg, Instant::now() + TIMEOUT)
    }
    pub(crate) fn connect_operation(
        cfg: &Config,
        operation: Option<crate::request::Operation>,
    ) -> io::Result<Self> {
        Self::connect_endpoint(cfg.obs_port, &cfg.obs_password, operation)
    }
    fn connect_endpoint(
        port: u16,
        password_path: &Path,
        operation: Option<crate::request::Operation>,
    ) -> io::Result<Self> {
        let deadline = operation
            .as_ref()
            .map_or(Ok(Instant::now() + TIMEOUT), |op| op.deadline(TIMEOUT))?;
        let mut obs = Self::connect_context(port, password_path, deadline, operation.clone())?;
        obs.operation = operation;
        Ok(obs)
    }
    #[cfg(test)]
    fn connect_until(cfg: &Config, deadline: Instant) -> io::Result<Self> {
        Self::connect_context(cfg.obs_port, &cfg.obs_password, deadline, None)
    }
    fn connect_context(
        port: u16,
        password_path: &Path,
        deadline: Instant,
        operation: Option<crate::request::Operation>,
    ) -> io::Result<Self> {
        let password = secret(password_path, 4096)?;
        if password.len() < 16 {
            return Err(err("OBS password is missing or too short"));
        }
        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
        let stream = TcpStream::connect_timeout(
            &addr.into(),
            remaining(deadline)?.min(Duration::from_secs(2)),
        )?;
        let stream = DeadlineStream {
            stream,
            deadline,
            operation,
        };
        let config = WebSocketConfig::default()
            .max_message_size(Some(LIMIT))
            .max_frame_size(Some(LIMIT))
            .max_write_buffer_size(LIMIT + 1024);
        let (socket, _) = tungstenite::client::client_with_config(
            format!("ws://127.0.0.1:{port}/"),
            stream,
            Some(config),
        )
        .map_err(|e| match e {
            tungstenite::HandshakeError::Failure(e) => redacted_websocket_error(e),
            _ => err("OBS WebSocket handshake interrupted"),
        })?;
        let mut obs = Self {
            operation: None,
            socket,
            seq: 0,
            failed: false,
        };
        let hello = obs.next(deadline)?;
        if hello["op"].as_u64() != Some(0) {
            return Err(err("OBS WebSocket v5 Hello required"));
        }
        let salt = auth_string(&hello, "salt")?;
        let challenge = auth_string(&hello, "challenge")?;
        let secret = base64_sha(format!("{password}{salt}").as_bytes());
        let proof = base64_sha(format!("{secret}{challenge}").as_bytes());
        obs.send(
            &json!({"op":1,"d":{"rpcVersion":1,"authentication":proof,"eventSubscriptions":0}}),
            deadline,
        )?;
        if obs.next(deadline)?["op"].as_u64() != Some(2) {
            return Err(err("OBS WebSocket authentication rejected"));
        }
        Ok(obs)
    }
    fn send(&mut self, value: &Value, deadline: Instant) -> io::Result<()> {
        remaining(deadline)?;
        let text = value.to_string();
        if text.len() > LIMIT {
            return Err(err("oversized OBS request"));
        }
        self.socket.get_mut().deadline = deadline;
        self.socket
            .send(Message::Text(text.into()))
            .map_err(redacted_websocket_error)
    }
    fn next(&mut self, deadline: Instant) -> io::Result<Value> {
        loop {
            remaining(deadline)?;
            self.socket.get_mut().deadline = deadline;
            match self.socket.read().map_err(redacted_websocket_error)? {
                Message::Text(text) => {
                    return serde_json::from_str(text.as_str()).map_err(|_| err("invalid OBS JSON"))
                }
                Message::Ping(_) | Message::Pong(_) => {}
                _ => return Err(err("OBS text message required")),
            }
        }
    }
    pub fn call(&mut self, name: &str, body: &str) -> io::Result<String> {
        let deadline = self
            .operation
            .as_ref()
            .map_or(Ok(Instant::now() + TIMEOUT), |op| op.deadline(TIMEOUT))?;
        self.call_until(name, body, deadline)
    }
    fn call_until(&mut self, name: &str, body: &str, deadline: Instant) -> io::Result<String> {
        if self.failed {
            return Err(err("OBS connection must be re-established"));
        }
        if body.len() > LIMIT / 2 || name.len() > 256 {
            return Err(err("oversized OBS request"));
        }
        let body: Value =
            serde_json::from_str(body).map_err(|_| err("invalid OBS request JSON"))?;
        if !body.is_object() {
            return Err(err("OBS request object required"));
        }
        self.seq = self
            .seq
            .checked_add(1)
            .ok_or_else(|| err("OBS request sequence exhausted"))?;
        let id = self.seq.to_string();
        let result = (|| {
            self.send(
                &json!({"op":6,"d":{"requestType":name,"requestId":id,"requestData":body}}),
                deadline,
            )?;
            for _ in 0..64 {
                let answer = self.next(deadline)?;
                if answer["op"].as_u64() != Some(7) {
                    continue;
                }
                if answer["d"]["requestId"].as_str() != Some(id.as_str()) {
                    continue;
                }
                if answer["d"]["requestStatus"]["result"].as_bool() != Some(true) {
                    let code = answer["d"]["requestStatus"]["code"]
                        .as_u64()
                        .map_or_else(|| "unknown".into(), |code| code.to_string());
                    return Err(err(&format!("OBS request {name} rejected (code {code})")));
                }
                return Ok(answer.to_string());
            }
            Err(err("OBS request lost amidst other events"))
        })();
        // A timed-out fragmented exchange must never be reused for another request.
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn has(&mut self, method: &str, list: &str, key: &str, needle: &str) -> io::Result<bool> {
        let response = response_data(&self.call(method, "{}")?)?;
        let entries = response[list]
            .as_array()
            .ok_or_else(|| err("OBS list required"))?;
        Ok(entries
            .iter()
            .any(|entry| entry[key].as_str() == Some(needle)))
    }
    fn input(&mut self, name: &str, kind: &str, settings: Value) -> io::Result<()> {
        if !self.has("GetInputList", "inputs", "inputName", name)? {
            self.call("CreateInput", &json!({"sceneName":"SplitCast","inputName":name,"inputKind":kind,"inputSettings":settings,"sceneItemEnabled":true}).to_string())?;
        } else {
            self.call(
                "SetInputSettings",
                &json!({"inputName":name,"inputSettings":settings,"overlay":false}).to_string(),
            )?;
        }
        Ok(())
    }
    pub(crate) fn scene_ready(&mut self) -> io::Result<bool> {
        self.has("GetSceneList", "scenes", "sceneName", "SplitCast")
    }
    fn scene_item(&mut self, source: &str) -> io::Result<i64> {
        let item = response_data(&self.call(
            "GetSceneItemId",
            &json!({"sceneName":"SplitCast","sourceName":source}).to_string(),
        )?)?;
        item["sceneItemId"]
            .as_i64()
            .ok_or_else(|| err("OBS scene item required"))
    }
    fn source_geometry_matches(
        &mut self,
        source: &str,
        width: u16,
        height: u16,
        crop: u16,
    ) -> io::Result<bool> {
        let id = self.scene_item(source)?;
        let response = response_data(&self.call(
            "GetSceneItemTransform",
            &json!({"sceneName":"SplitCast","sceneItemId":id}).to_string(),
        )?)?;
        let transform = &response["sceneItemTransform"];
        let matches = |key: &str, requested: u16| {
            transform[key].as_f64().is_some_and(|actual| {
                actual == f64::from(requested)
                    || (crop != 0 && actual == f64::from(requested.saturating_sub(crop).max(1)))
            })
        };
        Ok(matches("sourceWidth", width) && matches("sourceHeight", height))
    }
    pub(crate) fn source_geometry_ready(&mut self, cfg: &Config) -> io::Result<bool> {
        Ok(
            // Native GSR KMS can trim an even pixel pair at the screen edge.
            self.source_geometry_matches("GameFeed", cfg.width, cfg.height, 2)?
                && match &cfg.camera {
                    Some(camera) => {
                        self.source_geometry_matches("CameraFeed", camera.width, camera.height, 0)?
                    }
                    None => true,
                },
        )
    }
    pub(crate) fn configure(&mut self, cfg: &Config, layout: &SceneLayout) -> io::Result<()> {
        self.call("GetVersion", "{}")?;
        if !self.has("GetSceneList", "scenes", "sceneName", "SplitCast")? {
            self.call("CreateScene", "{\"sceneName\":\"SplitCast\"}")?;
        }
        let feed = |name| json!({"is_local_file":false,"input":format!("rtsp://127.0.0.1:{}/{name}",cfg.rtsp),"input_format":"rtsp","ffmpeg_options":"rtsp_transport=tcp","hw_decode":cfg.obs_video_mode == ObsVideoMode::Hardware,"buffering_mb":1});
        self.input("GameFeed", "ffmpeg_source", feed("game"))?;
        if let Some(camera) = layout.camera() {
            let mut settings = feed("camera");
            // A visibility edit must not restart or close the RTSP decoder.
            settings["restart_on_activate"] = false.into();
            settings["close_when_inactive"] = false.into();
            self.input("CameraFeed", "ffmpeg_source", settings)?;
            let id = self.scene_item("CameraFeed")?;
            self.camera_transform(id, camera)?;
        }
        self.input(
            "MicProcessed",
            "pulse_input_capture",
            json!({"device_id":format!("{}.monitor",cfg.processed_sink)}),
        )?;
        if cfg.separate_audio_tracks {
            for (name, game, microphone) in
                [("GameFeed", true, false), ("MicProcessed", false, true)]
            {
                self.call(
                    "SetInputAudioTracks",
                    &json!({
                        "inputName":name, "inputAudioTracks":{"1":true,"2":game,"3":microphone}
                    })
                    .to_string(),
                )?;
            }
        }
        self.call(
            "SetInputAudioSyncOffset",
            &json!({"inputName":"MicProcessed","inputAudioSyncOffset":cfg.mic_offset}).to_string(),
        )?;
        self.call(
            "SetInputAudioMonitorType",
            "{\"inputName\":\"MicProcessed\",\"monitorType\":\"OBS_MONITORING_TYPE_NONE\"}",
        )?;
        if let Some(text) = &layout.overlay().text {
            self.overlay(cfg.width, layout.overlay(), text)?;
        }
        self.call("SetCurrentProgramScene", "{\"sceneName\":\"SplitCast\"}")?;
        Ok(())
    }
    fn set_item(&mut self, method: &str, id: i64, field: &str, value: Value) -> io::Result<()> {
        let mut args = json!({"sceneName":"SplitCast","sceneItemId":id});
        args[field] = value;
        self.call(method, &args.to_string())?;
        Ok(())
    }

    fn camera_transform(&mut self, id: i64, camera: &CameraBox) -> io::Result<()> {
        self.set_item(
            "SetSceneItemTransform",
            id,
            "sceneItemTransform",
            fit_box(camera.x, camera.y, camera.size, 0),
        )
    }

    fn set_camera_visible(&mut self, id: i64, visible: bool) -> io::Result<()> {
        self.set_item(
            "SetSceneItemEnabled",
            id,
            "sceneItemEnabled",
            json!(visible),
        )
    }

    pub(crate) fn finish_start_layout(&mut self, layout: &SceneLayout) -> io::Result<()> {
        if layout.camera().is_some_and(|camera| !camera.visible) {
            // Let the configured source decode before hiding the scene item.
            let id = self.scene_item("CameraFeed")?;
            self.set_camera_visible(id, false)?;
        }
        Ok(())
    }

    fn overlay(&mut self, width: u16, overlay: &Overlay, text: &str) -> io::Result<()> {
        self.input(
            "SplitCastOverlay",
            "text_ft2_source_v2",
            json!({
                "text": text, "from_file": false,
                "font": {"face":"Noto Sans CJK JP","style":"Bold","size":32,"flags":1},
                "outline":true,"drop_shadow":true
            }),
        )?;
        let id = self.scene_item("SplitCastOverlay")?;
        self.set_item(
            "SetSceneItemTransform",
            id,
            "sceneItemTransform",
            fit_box(overlay.x, overlay.y, overlay_bounds(width), 5),
        )
    }

    fn scene_items(&mut self) -> io::Result<Vec<Value>> {
        let response =
            response_data(&self.call("GetSceneItemList", "{\"sceneName\":\"SplitCast\"}")?)?;
        match response {
            Value::Object(mut data) => match data.remove("sceneItems") {
                Some(Value::Array(items)) if items.len() <= 32 => Ok(items),
                _ => Err(err("invalid OBS scene item list")),
            },
            _ => Err(err("invalid OBS scene item list")),
        }
    }

    pub(crate) fn apply_layout(&mut self, canvas: Size, layout: &SceneLayout) -> io::Result<()> {
        if let Some(camera) = layout.camera() {
            let id = self.scene_item("CameraFeed")?;
            self.camera_transform(id, camera)?;
            self.set_camera_visible(id, camera.visible)?;
        }
        match &layout.overlay().text {
            Some(text) => self.overlay(canvas.width, layout.overlay(), text)?,
            None => {
                let items = self.scene_items()?;
                if scene_source(&items, "SplitCastOverlay")?.is_some() {
                    self.call("RemoveInput", "{\"inputName\":\"SplitCastOverlay\"}")?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn read_layout(&mut self, cfg: &Config) -> io::Result<Observation> {
        let video = response_data(&self.call("GetVideoSettings", "{}")?)?;
        let canvas = dimensions(&video, "baseWidth", "baseHeight")?;
        // The Agent-created profile is also the validation boundary of saved
        // drafts. An externally changed canvas cannot be persisted as ours.
        if canvas.width != cfg.width || canvas.height != cfg.height {
            return Err(err("owned OBS canvas differs from its configured profile"));
        }
        let items = self.scene_items()?;
        let game =
            scene_source(&items, "GameFeed")?.ok_or_else(|| err("owned OBS game item missing"))?;
        let game = dimensions(&game["sceneItemTransform"], "sourceWidth", "sourceHeight")?;
        let camera_item = scene_source(&items, "CameraFeed")?;
        let (camera, camera_layout) = if let Some(item) = camera_item {
            let transform = &item["sceneItemTransform"];
            let size = dimensions(transform, "boundsWidth", "boundsHeight")?;
            require_fit_box(transform, size, 0)?;
            (
                Some(dimensions(transform, "sourceWidth", "sourceHeight")?),
                json!({
                    "x":pixel(&transform["positionX"])?,"y":pixel(&transform["positionY"])?,
                    "width":size.width,"height":size.height,
                    "visible":item["sceneItemEnabled"].as_bool().ok_or_else(|| err("OBS camera visibility missing"))?
                }),
            )
        } else {
            (None, Value::Null)
        };
        let overlay = if let Some(item) = scene_source(&items, "SplitCastOverlay")? {
            let transform = &item["sceneItemTransform"];
            require_fit_box(transform, overlay_bounds(canvas.width), 5)?;
            if item["sceneItemEnabled"] != true {
                return Err(err("owned OBS overlay is not visible"));
            }
            let settings = response_data(
                &self.call("GetInputSettings", "{\"inputName\":\"SplitCastOverlay\"}")?,
            )?;
            let text = settings["inputSettings"]["text"]
                .as_str()
                .ok_or_else(|| err("OBS overlay text missing"))?;
            json!({"text":text,"x":pixel(&transform["positionX"])?,"y":pixel(&transform["positionY"])?})
        } else {
            json!({"text":null,"x":OVERLAY_ORIGIN,"y":OVERLAY_ORIGIN})
        };
        let layout = SceneLayout::parse(&json!({"camera":camera_layout,"overlay":overlay}))
            .and_then(|layout| {
                layout.validate(canvas, cfg.camera.is_some())?;
                Ok(layout)
            })
            .map_err(|_| err("OBS readback is outside the Studio layout contract"))?;
        Ok(Observation {
            canvas,
            game,
            camera,
            layout,
        })
    }

    pub(crate) fn configure_output(
        &mut self,
        cfg: &Config,
        output: crate::Output,
    ) -> io::Result<()> {
        if output == crate::Output::Recording {
            return Ok(());
        }
        let twitch = cfg
            .twitch
            .as_ref()
            .ok_or_else(|| err("Twitch is not configured on production"))?;
        if output == crate::Output::TwitchLive && !twitch.allow_public {
            return Err(err("public Twitch output is disabled on production"));
        }
        let mut key = secret(&twitch.key_file, 512)?;
        if !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
        {
            return Err(err("invalid Twitch stream key"));
        }
        if output == crate::Output::TwitchTest {
            key.push_str("?bandwidthtest=true");
        }
        self.call(
            "SetStreamServiceSettings",
            &json!({
                "streamServiceType":"rtmp_custom",
                "streamServiceSettings":{
                    "server":format!("rtmps://{}/app/", twitch.ingest_host),
                    "key":key,"use_auth":false
                }
            })
            .to_string(),
        )?;
        Ok(())
    }
    pub fn start(&mut self, output: crate::Output) -> io::Result<()> {
        self.call(
            if output == crate::Output::Recording {
                "StartRecord"
            } else {
                "StartStream"
            },
            "{}",
        )
        .map(|_| ())
    }
    pub fn stop(&mut self, output: crate::Output) -> io::Result<()> {
        let deadline = self
            .operation
            .as_ref()
            .map_or(Ok(Instant::now() + TIMEOUT), |op| op.deadline(TIMEOUT))?;
        let mut requested = false;
        loop {
            let response =
                response_data(&self.call_until(status_request(output), "{}", deadline)?)?;
            match response["outputActive"].as_bool() {
                Some(false) => return Ok(()),
                Some(true) => {}
                None => return Err(err("OBS outputActive boolean required")),
            }
            if !requested {
                self.call_until(
                    if output == crate::Output::Recording {
                        "StopRecord"
                    } else {
                        "StopStream"
                    },
                    "{}",
                    deadline,
                )?;
                requested = true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn observe(&mut self, output: crate::Output) -> io::Result<OutputStatus> {
        let response = response_data(&self.call(status_request(output), "{}")?)?;
        OutputStatus::from_response(output, &response)
    }
    pub fn preview(&mut self) -> io::Result<String> {
        let image = self.screenshot(960, 540)?;
        Ok(json!({"image":image}).to_string())
    }

    pub(crate) fn preview_small(&mut self) -> io::Result<String> {
        const WIDTH: u32 = 480;
        const HEIGHT: u32 = 270;
        let image = self.screenshot(WIDTH, HEIGHT)?;
        let png = image
            .strip_prefix("data:image/png;base64,")
            .filter(|_| image.len() <= 768 * 1024)
            .and_then(|encoded| STANDARD.decode(encoded).ok())
            .ok_or_else(|| err("invalid or oversized Studio preview"))?;
        if !png.starts_with(b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR") || png.len() < 33 {
            return Err(err("invalid Studio preview PNG header"));
        }
        let width = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
        let height = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
        if !(1..=WIDTH).contains(&width) || !(1..=HEIGHT).contains(&height) {
            return Err(err("Studio preview dimensions exceed the requested bounds"));
        }
        Ok(json!({"image":image}).to_string())
    }

    fn screenshot(&mut self, width: u32, height: u32) -> io::Result<String> {
        let response = response_data(&self.call("GetSourceScreenshot", &json!({
            "sourceName":"SplitCast","imageFormat":"png","imageWidth":width,"imageHeight":height
        }).to_string())?)?;
        response["imageData"]
            .as_str()
            .filter(|image| image.starts_with("data:image/png;base64,"))
            .map(str::to_owned)
            .ok_or_else(|| err("OBS returned an invalid scene screenshot"))
    }
}

fn fit_box(x: u16, y: u16, size: Size, alignment: u16) -> Value {
    json!({
        "alignment":5,"positionX":x,"positionY":y,
        "boundsType":"OBS_BOUNDS_SCALE_INNER","boundsAlignment":alignment,
        "boundsWidth":size.width,"boundsHeight":size.height,
        "rotation":0,"scaleX":1.0,"scaleY":1.0,
        "cropTop":0,"cropBottom":0,"cropLeft":0,"cropRight":0,"cropToBounds":false
    })
}

fn overlay_bounds(width: u16) -> Size {
    Size {
        width: (width * 3) / 4 - 32,
        height: 64,
    }
}

fn scene_source<'a>(items: &'a [Value], source: &str) -> io::Result<Option<&'a Value>> {
    let mut matches = items.iter().filter(|item| item["sourceName"] == source);
    let first = matches.next();
    if matches.next().is_some() {
        return Err(err("owned OBS source has duplicate scene items"));
    }
    Ok(first)
}

fn pixel(value: &Value) -> io::Result<u16> {
    value
        .as_f64()
        .filter(|number| (0.0..=f64::from(u16::MAX)).contains(number) && number.fract() == 0.0)
        .map(|number| number as u16)
        .ok_or_else(|| err("OBS geometry is missing or not an integer pixel value"))
}

fn dimensions(value: &Value, width: &str, height: &str) -> io::Result<Size> {
    let size = Size {
        width: pixel(&value[width])?,
        height: pixel(&value[height])?,
    };
    if size.width == 0 || size.height == 0 {
        return Err(err("OBS source geometry is unavailable"));
    }
    Ok(size)
}

fn require_fit_box(transform: &Value, size: Size, alignment: u16) -> io::Result<()> {
    if transform["alignment"] != 5
        || transform["boundsAlignment"] != alignment
        || (transform["boundsType"] != "OBS_BOUNDS_SCALE_INNER" && transform["boundsType"] != 2)
        || dimensions(transform, "boundsWidth", "boundsHeight")? != size
        || transform["rotation"].as_f64() != Some(0.0)
        || ["cropLeft", "cropRight", "cropTop", "cropBottom"]
            .iter()
            .any(|field| transform[*field] != 0)
        || transform["cropToBounds"] == true
    {
        return Err(err(
            "owned OBS item transform is outside the Studio layout contract",
        ));
    }
    Ok(())
}

fn status_request(output: crate::Output) -> &'static str {
    match output {
        crate::Output::Recording => "GetRecordStatus",
        crate::Output::TwitchTest | crate::Output::TwitchLive => "GetStreamStatus",
    }
}

fn launch_args(cfg: &Config, base: &Path) -> io::Result<Vec<String>> {
    let _port = TcpListener::bind((Ipv4Addr::LOCALHOST, cfg.obs_port))
        .map_err(|_| err("OBS port is already occupied"))?;
    let studio = base.join("obs-studio");
    let password = secret(&cfg.obs_password, 4096)?;
    if password.len() < 16 {
        return Err(err("OBS auth password must be at least 16 characters"));
    }
    private_directory(&cfg.state_dir)?;
    atomic_file(&studio.join("plugin_config/obs-websocket/config.json"), json!({"server_enabled":true,"server_port":cfg.obs_port,"auth_required":true,"server_password":password,"first_load":false}).to_string())?;
    fs::create_dir_all(&cfg.recording_dir)?;
    if cfg.recording_dir.to_string_lossy().contains(['\n', '\r']) {
        return Err(err("invalid recording directory"));
    }
    let encoder = match cfg.obs_video_mode {
        ObsVideoMode::Hardware => "obs_nvenc_h264_tex",
        ObsVideoMode::Software => "obs_x264",
    };
    let recording_tracks = if cfg.separate_audio_tracks {
        0b111
    } else {
        0b001
    };
    let profile = format!("[General]\nName=SplitCast\n[Video]\nBaseCX={width}\nBaseCY={height}\nOutputCX={width}\nOutputCY={height}\nFPSType=0\nFPSCommon={fps}\n[Output]\nMode=Advanced\nReconnect=false\n[AdvOut]\nEncoder={encoder}\nRecType=Standard\nRecFilePath={}\nRecFormat2=mkv\nRecEncoder={encoder}\nTrackIndex=1\nRecTracks={recording_tracks}\nRescale=false\n[Audio]\nSampleRate=48000\nChannelSetup=Stereo\n", cfg.recording_dir.display(),width=cfg.width,height=cfg.height,fps=cfg.fps);
    atomic_file(&studio.join("basic/profiles/SplitCast/basic.ini"), &profile)?;
    atomic_file(
        &studio.join("basic/profiles/SplitCast/streamEncoder.json"),
        json!({"rate_control":"CBR","bitrate":cfg.obs_bitrate,"keyint_sec":2}).to_string(),
    )?;
    atomic_file(&studio.join("user.ini"), "[Basic]\nProfile=SplitCast\nProfileDir=SplitCast\nSceneCollection=SplitCast\nSceneCollectionFile=SplitCast\n[General]\nFirstRun=true\n")?;
    atomic_file(&studio.join("basic/scenes/SplitCast.json"), "{\"name\":\"SplitCast\",\"current_scene\":\"SplitCast\",\"current_program_scene\":\"SplitCast\",\"scene_order\":[{\"name\":\"SplitCast\"}],\"sources\":[{\"name\":\"SplitCast\",\"id\":\"scene\",\"versioned_id\":\"scene\",\"settings\":{\"items\":[]},\"mixers\":0}],\"groups\":[],\"transitions\":[]}")?;
    Ok(vec![
        "env".into(),
        format!("XDG_CONFIG_HOME={}", base.display()),
        cfg.obs_binary.to_string_lossy().to_string(),
        "--multi".into(),
        "--disable-shutdown-check".into(),
        "--profile".into(),
        "SplitCast".into(),
        "--collection".into(),
        "SplitCast".into(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
    use std::path::{Path, PathBuf};
    use std::thread;

    struct Fixture {
        dir: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let dir = PathBuf::from("/tmp/opencode").join(format!(
                "native-obs-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            private_directory(&dir).unwrap();
            atomic_file(&dir.join("password"), "a-private-password-long-enough").unwrap();
            Self { dir }
        }
        fn config(&self, port: u16) -> Config {
            config(&self.dir, port)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }
    #[test]
    fn profile_uses_the_selected_video_geometry() {
        let fixture = Fixture::new();
        // This configuration-only test starts no OBS. Let launch_args perform
        // its own ephemeral bind, without releasing a reservation first.
        let mut cfg = fixture.config(0);
        cfg.width = 1280;
        cfg.height = 720;
        cfg.fps = 30;
        launch_args(&cfg, &cfg.state_dir.join("obs-config")).unwrap();
        let profile = fs::read_to_string(
            cfg.state_dir
                .join("obs-config/obs-studio/basic/profiles/SplitCast/basic.ini"),
        )
        .unwrap();
        for setting in [
            "BaseCX=1280\n",
            "BaseCY=720\n",
            "OutputCX=1280\n",
            "OutputCY=720\n",
            "FPSCommon=30\n",
        ] {
            assert!(
                profile.contains(setting),
                "missing selected video setting {setting:?}"
            );
        }
    }
    #[test]
    fn profile_uses_selected_encoder_for_stream_and_recording() {
        let fixture = Fixture::new();
        let mut cfg = fixture.config(0);
        for (mode, encoder) in [
            (ObsVideoMode::Hardware, "obs_nvenc_h264_tex"),
            (ObsVideoMode::Software, "obs_x264"),
        ] {
            cfg.obs_video_mode = mode;
            launch_args(&cfg, &cfg.state_dir.join("obs-config")).unwrap();
            let profile = fs::read_to_string(
                cfg.state_dir
                    .join("obs-config/obs-studio/basic/profiles/SplitCast/basic.ini"),
            )
            .unwrap();
            assert!(profile
                .lines()
                .any(|line| line == format!("Encoder={encoder}")));
            assert!(profile
                .lines()
                .any(|line| line == format!("RecEncoder={encoder}")));
        }
    }
    fn config(dir: &Path, port: u16) -> Config {
        let mut value: Value =
            serde_json::from_str(include_str!("../tests/agent-example.json")).unwrap();
        value["role"] = json!("production");
        value["peer"]["state_dir"] = json!(dir);
        value["capture"]["width"] = json!(1920);
        value["capture"]["height"] = json!(1080);
        value["obs"]["password_file"] = json!(dir.join("password"));
        value["obs"]["recording_dir"] = json!(dir.join("recordings"));
        let mut cfg = Config::parse(value.to_string().as_bytes()).unwrap();
        cfg.obs_port = port; // zero requests an ephemeral port in profile-only tests
        cfg
    }
    fn read(socket: &mut WebSocket<TcpStream>) -> Value {
        serde_json::from_str(socket.read().unwrap().to_text().unwrap()).unwrap()
    }
    fn send(socket: &mut WebSocket<TcpStream>, value: Value) {
        socket
            .send(Message::Text(value.to_string().into()))
            .unwrap();
    }
    fn answer(socket: &mut WebSocket<TcpStream>, request: &Value, data: Value) {
        send(
            socket,
            json!({"op":7,"d":{"requestId":request["d"]["requestId"],"requestStatus":{"result":true},"responseData":data}}),
        );
    }
    fn server(
        f: impl FnOnce(WebSocket<TcpStream>) + Send + 'static,
    ) -> (u16, thread::JoinHandle<()>) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let thread = thread::spawn(move || f(accept_obs(&listener)));
        (port, thread)
    }
    fn accept_obs(listener: &TcpListener) -> WebSocket<TcpStream> {
        let (stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(TIMEOUT)).unwrap();
        stream.set_write_timeout(Some(TIMEOUT)).unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        send(
            &mut socket,
            json!({"op":0,"d":{"authentication":{"salt":"salt\"","challenge":"challenge\\"}}}),
        );
        let identify = read(&mut socket);
        let secret = base64_sha(b"a-private-password-long-enoughsalt\"");
        assert_eq!(
            identify["d"]["authentication"],
            base64_sha(format!("{secret}challenge\\").as_bytes())
        );
        assert_eq!(identify["d"]["eventSubscriptions"], 0);
        send(&mut socket, json!({"op":2,"d":{"negotiatedRpcVersion":1}}));
        socket
    }

    #[test]
    fn configure_sends_selected_decode_mode_over_websocket() {
        for (mode, hw_decode) in [
            (ObsVideoMode::Hardware, true),
            (ObsVideoMode::Software, false),
        ] {
            let fixture = Fixture::new();
            let (port, thread) = server(move |mut socket| {
                let mut feeds = Vec::new();
                loop {
                    let request = read(&mut socket);
                    let method = request["d"]["requestType"].as_str().unwrap();
                    if method == "CreateInput" {
                        let data = &request["d"]["requestData"];
                        if data["inputKind"] == "ffmpeg_source" {
                            assert_eq!(data["inputSettings"]["hw_decode"], hw_decode);
                            feeds.push(data["inputName"].as_str().unwrap().to_owned());
                        }
                    }
                    let data = match method {
                        "GetSceneList" => json!({"scenes":[{"sceneName":"SplitCast"}]}),
                        "GetInputList" => json!({"inputs":[]}),
                        _ => json!({}),
                    };
                    answer(&mut socket, &request, data);
                    if method == "SetCurrentProgramScene" {
                        break;
                    }
                }
                assert_eq!(feeds, ["GameFeed"]);
            });
            let mut cfg = fixture.config(port);
            cfg.obs_video_mode = mode;
            Obs::connect(&cfg)
                .unwrap()
                .configure(&cfg, &cfg.default_scene)
                .unwrap();
            thread.join().unwrap();
        }
    }

    #[test]
    fn game_readiness_requires_decoded_source_geometry() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            for (width, height) in [(0, 0), (1280, 720), (1920, 1080)] {
                let request = read(&mut socket);
                assert_eq!(request["d"]["requestType"], "GetSceneItemId");
                answer(&mut socket, &request, json!({"sceneItemId":7}));
                let request = read(&mut socket);
                assert_eq!(request["d"]["requestType"], "GetSceneItemTransform");
                assert_eq!(request["d"]["requestData"]["sceneItemId"], 7);
                answer(
                    &mut socket,
                    &request,
                    json!({"sceneItemTransform":{"sourceWidth":width,"sourceHeight":height}}),
                );
            }
        });
        let mut cfg = fixture.config(port);
        cfg.width = 1280;
        cfg.height = 720;
        let mut obs = Obs::connect(&cfg).unwrap();
        assert!(!obs.source_geometry_ready(&cfg).unwrap());
        assert!(obs.source_geometry_ready(&cfg).unwrap());
        assert!(!obs.source_geometry_ready(&cfg).unwrap());
        thread.join().unwrap();
    }

    #[test]
    fn compound_configuration_uses_one_operation_budget() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            while let Ok(message) = socket.read() {
                let request: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
                thread::sleep(Duration::from_millis(40));
                let response = json!({"op":7,"d":{"requestId":request["d"]["requestId"],"requestStatus":{"result":true},"responseData":{"scenes":[{"sceneName":"SplitCast"}],"inputs":[{"inputName":"GameFeed"},{"inputName":"MicProcessed"}]}}});
                if socket
                    .send(Message::Text(response.to_string().into()))
                    .is_err()
                {
                    break;
                }
            }
        });
        let cfg = fixture.config(port);
        let operation = crate::request::Operation::new(
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            None,
            Duration::from_millis(140),
        );
        let mut obs = Obs::connect_operation(&cfg, Some(operation)).unwrap();
        assert_eq!(
            obs.configure(&cfg, &cfg.default_scene).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        drop(obs);
        thread.join().unwrap();
    }
    #[test]
    fn cancellation_between_configuration_calls_prevents_the_next_rpc() {
        let fixture = Fixture::new();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let shutdown = std::sync::Arc::clone(&cancel);
        let (port, thread) = server(move |mut socket| {
            let request = read(&mut socket);
            assert_eq!(request["d"]["requestType"], "GetVersion");
            shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
            answer(&mut socket, &request, json!({}));
            assert!(
                socket.read().is_err(),
                "configuration sent another RPC after cancellation"
            );
        });
        let cfg = fixture.config(port);
        let operation = crate::request::Operation::new(cancel, None, Duration::from_secs(2));
        let mut obs = Obs::connect_operation(&cfg, Some(operation)).unwrap();
        assert_eq!(
            obs.configure(&cfg, &cfg.default_scene).unwrap_err().kind(),
            io::ErrorKind::Interrupted
        );
        drop(obs);
        thread.join().unwrap();
    }
    #[test]
    fn wrong_ids_are_ignored_and_result_must_be_boolean() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            let request = read(&mut socket);
            send(
                &mut socket,
                json!({"op":7,"d":{"requestId":"wrong","requestStatus":{"result":true}}}),
            );
            send(&mut socket, json!({"op":5}));
            send(
                &mut socket,
                json!({"op":7,"d":{"requestId":request["d"]["requestId"],"requestStatus":{"result":"true"}}}),
            );
        });
        let mut obs = Obs::connect(&fixture.config(port)).unwrap();
        assert_eq!(
            obs.call("GetVersion", "{}").unwrap_err().to_string(),
            "OBS request GetVersion rejected (code unknown)"
        );
        assert!(obs
            .call("GetVersion", "{}")
            .unwrap_err()
            .to_string()
            .contains("re-established"));
        thread.join().unwrap();
    }
    #[test]
    fn stalled_response_and_events_share_one_deadline() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            read(&mut socket);
            for _ in 0..20 {
                if socket.send(Message::Text("{\"op\":5}".into())).is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }
        });
        let mut obs = Obs::connect(&fixture.config(port)).unwrap();
        let start = Instant::now();
        assert_eq!(
            obs.call_until("GetVersion", "{}", start + Duration::from_millis(100))
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(start.elapsed() < Duration::from_millis(500));
        drop(obs);
        thread.join().unwrap();
    }
    #[test]
    fn dripfed_partial_frame_cannot_extend_deadline() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            read(&mut socket);
            let stream = socket.get_mut();
            stream.write_all(&[0x81, 126, 0, 200]).unwrap();
            for _ in 0..200 {
                if stream.write_all(b"x").is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        let mut obs = Obs::connect(&fixture.config(port)).unwrap();
        let start = Instant::now();
        assert_eq!(
            obs.call_until("GetVersion", "{}", start + Duration::from_millis(100))
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(start.elapsed() < Duration::from_millis(500));
        drop(obs);
        thread.join().unwrap();
    }
    #[test]
    fn stalled_response_is_bounded() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            read(&mut socket);
            thread::sleep(Duration::from_millis(250));
        });
        let mut obs = Obs::connect(&fixture.config(port)).unwrap();
        let start = Instant::now();
        assert_eq!(
            obs.call_until("GetVersion", "{}", start + Duration::from_millis(100))
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(start.elapsed() < Duration::from_millis(500));
        drop(obs);
        thread.join().unwrap();
    }
    #[test]
    fn dripfed_http_upgrade_is_bounded() {
        let fixture = Fixture::new();
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let cfg = fixture.config(listener.local_addr().unwrap().port());
        let thread = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(TIMEOUT)).unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            for byte in b"HTTP/1.1 101 Switching Protocols\r\n" {
                if stream.write_all(&[*byte]).is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        let start = Instant::now();
        let result = Obs::connect_until(&cfg, start + Duration::from_millis(100));
        assert_eq!(result.err().unwrap().kind(), io::ErrorKind::TimedOut);
        assert!(start.elapsed() < Duration::from_millis(500));
        thread.join().unwrap();
    }
    #[test]
    fn unauthenticated_server_is_rejected() {
        let fixture = Fixture::new();
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let cfg = fixture.config(listener.local_addr().unwrap().port());
        let thread = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            send(&mut socket, json!({"op":0,"d":{"rpcVersion":1}}));
        });
        assert!(Obs::connect(&cfg)
            .err()
            .unwrap()
            .to_string()
            .contains("authentication string required"));
        thread.join().unwrap();
    }
    #[test]
    fn blocked_tcp_writes_share_one_deadline() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (_peer, _) = listener.accept().unwrap();
        let start = Instant::now();
        let mut stream = DeadlineStream {
            stream,
            deadline: start + Duration::from_millis(100),
            operation: None,
        };
        assert_eq!(
            stream
                .write_all(&vec![0; 32 * 1024 * 1024])
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(start.elapsed() < Duration::from_millis(500));
    }
    #[test]
    fn dripfed_continuations_cannot_extend_deadline() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            read(&mut socket);
            let stream = socket.get_mut();
            stream.write_all(&[0x01, 1, b'{']).unwrap();
            for _ in 0..100 {
                if stream.write_all(&[0x00, 1, b' ']).is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        let mut obs = Obs::connect(&fixture.config(port)).unwrap();
        let start = Instant::now();
        assert_eq!(
            obs.call_until("GetVersion", "{}", start + Duration::from_millis(100))
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(start.elapsed() < Duration::from_millis(500));
        drop(obs);
        thread.join().unwrap();
    }
    #[test]
    fn oversized_frame_is_rejected_before_payload() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            read(&mut socket);
            socket
                .get_mut()
                .write_all(&[0x81, 127, 0, 0, 0, 0, 0, 0x40, 0, 1])
                .unwrap();
        });
        let mut obs = Obs::connect(&fixture.config(port)).unwrap();
        assert!(obs
            .call("GetVersion", "{}")
            .unwrap_err()
            .to_string()
            .contains("oversized"));
        thread.join().unwrap();
    }
    #[test]
    fn oversized_fragmented_message_is_rejected() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            read(&mut socket);
            let stream = socket.get_mut();
            stream
                .write_all(&[0x01, 127, 0, 0, 0, 0, 0, 0x20, 0, 0])
                .unwrap();
            stream.write_all(&vec![b'x'; LIMIT / 2]).unwrap();
            stream
                .write_all(&[0x80, 127, 0, 0, 0, 0, 0, 0x20, 0, 1])
                .unwrap();
            let _ = stream.write_all(&vec![b'x'; LIMIT / 2 + 1]);
        });
        let mut obs = Obs::connect(&fixture.config(port)).unwrap();
        assert!(obs
            .call("GetVersion", "{}")
            .unwrap_err()
            .to_string()
            .contains("oversized"));
        drop(obs);
        thread.join().unwrap();
    }
    #[test]
    fn exact_names_only_not_settings_or_substrings() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            let request = read(&mut socket);
            answer(
                &mut socket,
                &request,
                json!({"scenes":[{"sceneName":"SplitCastBackup"},{"sceneName":"Other","settings":"SplitCast"}]}),
            );
            let request = read(&mut socket);
            answer(
                &mut socket,
                &request,
                json!({"inputs":[{"inputName":"GameFeedBackup"},{"inputName":"Other","settings":"GameFeed"}]}),
            );
            let request = read(&mut socket);
            answer(
                &mut socket,
                &request,
                json!({"scenes":[{"sceneName":"SplitCast"}]}),
            );
        });
        let mut obs = Obs::connect(&fixture.config(port)).unwrap();
        assert!(!obs
            .has("GetSceneList", "scenes", "sceneName", "SplitCast")
            .unwrap());
        assert!(!obs
            .has("GetInputList", "inputs", "inputName", "GameFeed")
            .unwrap());
        assert!(obs
            .has("GetSceneList", "scenes", "sceneName", "SplitCast")
            .unwrap());
        thread.join().unwrap();
    }
    #[test]
    fn output_status_reports_progress() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            let request = read(&mut socket);
            assert_eq!(request["d"]["requestType"], "GetRecordStatus");
            answer(
                &mut socket,
                &request,
                json!({"outputActive":true,"outputBytes":123,"outputDuration":99}),
            );
        });
        let cfg = fixture.config(port);
        assert_eq!(
            read_output_status(port, &cfg.obs_password, crate::Output::Recording, None).unwrap(),
            OutputStatus {
                active: true,
                bytes: 123,
                frames: None,
                duration_ms: Some(99)
            }
        );
        thread.join().unwrap();
    }

    #[test]
    fn output_status_requires_typed_active_and_unsigned_counters() {
        let valid = json!({"outputActive": true, "outputBytes": 4, "outputTotalFrames": 3});
        for (field, malformed, expected) in [
            (
                "outputActive",
                json!(1),
                "OBS outputActive boolean required",
            ),
            (
                "outputBytes",
                json!(-1),
                "OBS outputBytes unsigned counter required",
            ),
            (
                "outputBytes",
                json!(4.0),
                "OBS outputBytes unsigned counter required",
            ),
            (
                "outputBytes",
                json!("4"),
                "OBS outputBytes unsigned counter required",
            ),
            (
                "outputTotalFrames",
                json!(-1),
                "OBS stream outputTotalFrames unsigned counter required",
            ),
            (
                "outputTotalFrames",
                json!(3.0),
                "OBS stream outputTotalFrames unsigned counter required",
            ),
            (
                "outputTotalFrames",
                json!("3"),
                "OBS stream outputTotalFrames unsigned counter required",
            ),
        ] {
            let mut status = valid.clone();
            status[field] = malformed;
            for output in [crate::Output::TwitchTest, crate::Output::TwitchLive] {
                assert_eq!(
                    OutputStatus::from_response(output, &status)
                        .unwrap_err()
                        .to_string(),
                    expected
                );
            }
        }
        for (field, expected) in [
            ("outputActive", "OBS outputActive boolean required"),
            ("outputBytes", "OBS outputBytes unsigned counter required"),
            (
                "outputTotalFrames",
                "OBS stream outputTotalFrames unsigned counter required",
            ),
        ] {
            let mut status = valid.clone();
            status.as_object_mut().unwrap().remove(field);
            assert_eq!(
                OutputStatus::from_response(crate::Output::TwitchTest, &status)
                    .unwrap_err()
                    .to_string(),
                expected
            );
        }
    }

    #[test]
    fn recording_progress_uses_bytes_and_reports_unknown_frames() {
        let sample = |active, bytes, duration: Value| {
            OutputStatus::from_response(
                crate::Output::Recording,
                &json!({
                    "outputActive": active, "outputBytes": bytes,
                    "outputDuration": duration, "outputTotalFrames": "ignored",
                }),
            )
            .unwrap()
        };
        assert!(!sample(false, 3_u64, Value::Null).ready());
        assert!(!sample(true, 0, Value::Null).ready());
        assert!(OutputProgress::start(sample(true, 0, Value::Null)).is_none());

        let baseline = sample(true, 3, json!(100));
        assert!(baseline.ready());
        let mut progress = OutputProgress::start(baseline).unwrap();
        let status = json!({
            "active": true, "bytes": 3, "frames": null,
            "frame_health": "unknown", "duration_ms": 100,
        });
        assert_eq!(progress.snapshot().status(), status);
        for (active, bytes) in [(false, 10), (true, 3), (true, 2)] {
            assert!(progress
                .observe(sample(active, bytes, Value::Null))
                .is_none());
            assert_eq!(progress.snapshot().status(), status);
        }
        assert_eq!(
            progress
                .observe(sample(true, 4, json!("optional")))
                .unwrap()
                .status(),
            json!({
                "active": true, "bytes": 4, "frames": null,
                "frame_health": "unknown", "duration_ms": null,
            })
        );
    }

    #[test]
    fn stream_progress_can_move_without_moving_the_attempted_output() {
        let output = crate::Output::TwitchTest;
        let sample = |bytes: u64, frames: u64| {
            OutputStatus::from_response(
                output,
                &json!({"outputActive": true, "outputBytes": bytes, "outputTotalFrames": frames}),
            )
            .unwrap()
        };
        let profile = SessionProfile::test_attempt(PathBuf::new(), output);
        assert!(!sample(10, 0).ready());
        assert!(!sample(0, 2).ready());
        assert!(OutputProgress::start(sample(10, 0)).is_none());
        let mut progress = OutputProgress::start(sample(10, 2)).unwrap();
        let baseline = json!({
            "active": true, "bytes": 10, "frames": 2,
            "frame_health": "pending", "duration_ms": null,
        });
        assert_eq!(progress.snapshot().status(), baseline);

        assert_eq!(profile.requested_output(), Some(output));
        for (bytes, frames) in [(20, 2), (20, 1), (10, 4), (9, 4)] {
            assert!(progress.observe(sample(bytes, frames)).is_none());
            assert_eq!(progress.snapshot().status(), baseline);
        }
        assert_eq!(
            progress.observe(sample(11, 3)).unwrap().status(),
            json!({
                "active": true, "bytes": 11, "frames": 3,
                "frame_health": "progressing", "duration_ms": null,
            })
        );
    }

    #[test]
    fn start_requests_private_recording() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            let request = read(&mut socket);
            assert_eq!(request["d"]["requestType"], "StartRecord");
            answer(&mut socket, &request, json!({}));
        });
        let cfg = fixture.config(port);
        let mut obs = Obs::connect(&cfg).unwrap();
        obs.start(crate::Output::Recording).unwrap();
        thread.join().unwrap();
    }
    #[test]
    fn stop_waits_for_recording_finalization_before_returning() {
        let fixture = Fixture::new();
        let (port, thread) = server(|mut socket| {
            for (method, data) in [
                ("GetRecordStatus", json!({"outputActive":true})),
                ("StopRecord", json!({})),
                ("GetRecordStatus", json!({"outputActive":true})),
                ("GetRecordStatus", json!({"outputActive":false})),
            ] {
                let request = read(&mut socket);
                assert_eq!(request["d"]["requestType"], method);
                answer(&mut socket, &request, data);
            }
        });
        Obs::connect(&fixture.config(port))
            .unwrap()
            .stop(crate::Output::Recording)
            .unwrap();
        thread.join().unwrap();
    }
    #[test]
    fn twitch_test_selects_tls_and_stop_reconciles_a_lost_start_reply() {
        use crate::Output::{TwitchLive, TwitchTest};
        let fixture = Fixture::new();
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let mut cfg = fixture.config(listener.local_addr().unwrap().port());
        let key_file = fixture.dir.join("stream-key");
        atomic_file(&key_file, "live_fake_key").unwrap();
        cfg.twitch = Some(crate::config::Twitch {
            ingest_host: "apn10.contribute.live-video.net".into(),
            key_file,
            allow_public: false,
        });
        let thread = thread::spawn(move || {
            let mut socket = accept_obs(&listener);
            let request = read(&mut socket);
            assert_eq!(request["d"]["requestType"], "SetStreamServiceSettings");
            assert_eq!(
                request["d"]["requestData"],
                json!({
                    "streamServiceType":"rtmp_custom",
                    "streamServiceSettings":{"server":"rtmps://apn10.contribute.live-video.net/app/",
                        "key":"live_fake_key?bandwidthtest=true","use_auth":false}
                })
            );
            answer(&mut socket, &request, json!({}));
            assert_eq!(read(&mut socket)["d"]["requestType"], "StartStream");
            drop(socket);

            let mut socket = accept_obs(&listener);
            for (method, data) in [
                ("GetStreamStatus", json!({"outputActive":true})),
                ("StopStream", json!({})),
                ("GetStreamStatus", json!({"outputActive":false})),
            ] {
                let request = read(&mut socket);
                assert_eq!(request["d"]["requestType"], method);
                answer(&mut socket, &request, data);
            }
        });
        let mut obs = Obs::connect(&cfg).unwrap();
        assert!(obs.configure_output(&cfg, TwitchLive).is_err());
        obs.configure_output(&cfg, TwitchTest).unwrap();
        assert!(obs.start(TwitchTest).is_err());
        Obs::connect(&cfg).unwrap().stop(TwitchTest).unwrap();
        thread.join().unwrap();
    }
    #[test]
    fn launch_refuses_occupied_port() {
        let fixture = Fixture::new();
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        assert!(launch_args(
            &fixture.config(listener.local_addr().unwrap().port()),
            &fixture.dir.join("obs-config"),
        )
        .unwrap_err()
        .to_string()
        .contains("occupied"));
    }
    #[test]
    fn launch_refuses_public_credentials_and_keeps_generated_files_private() {
        let fixture = Fixture::new();
        let cfg = fixture.config(0);
        fs::set_permissions(&cfg.obs_password, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(launch_args(&cfg, &cfg.state_dir.join("obs-config"),).is_err());
        fs::set_permissions(&cfg.obs_password, fs::Permissions::from_mode(0o600)).unwrap();
        launch_args(&cfg, &cfg.state_dir.join("obs-config")).unwrap();
        let credentials = cfg
            .state_dir
            .join("obs-config/obs-studio/plugin_config/obs-websocket/config.json");
        assert_eq!(fs::metadata(&credentials).unwrap().mode() & 0o777, 0o600);
        assert_eq!(
            fs::metadata(credentials.parent().unwrap()).unwrap().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::read_to_string(&cfg.obs_password).unwrap(),
            "a-private-password-long-enough"
        );
    }
    #[test]
    fn atomic_state_rejects_symlinks_and_ignores_predictable_temp_symlink() {
        let fixture = Fixture::new();
        let victim = fixture.dir.join("victim");
        fs::write(&victim, "untouched").unwrap();
        let state = fixture.dir.join("state.json");
        symlink(&victim, state.with_extension("tmp")).unwrap();
        atomic_file(&state, "private").unwrap();
        assert_eq!(fs::read_to_string(&victim).unwrap(), "untouched");
        assert_eq!(fs::metadata(&state).unwrap().mode() & 0o777, 0o600);
        fs::remove_file(&state).unwrap();
        symlink(&victim, &state).unwrap();
        assert!(atomic_file(&state, "private").is_err());
        let alias = fixture.dir.join("alias");
        symlink(&fixture.dir, &alias).unwrap();
        assert!(atomic_file(&alias.join("other"), "private").is_err());
        assert_eq!(fs::read_to_string(victim).unwrap(), "untouched");
    }
}
