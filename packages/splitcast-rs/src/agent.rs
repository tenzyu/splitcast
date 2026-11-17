use crate::protocol::{Command, Invocation, MicAction, OwnerId};
use crate::workers::{Worker, Workers};
use crate::{config, control, media, obs};
use crate::{Lifecycle, Output, Role, Session};
use serde_json::{json, Value};
use std::fs;
use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};

fn err(s: &str) -> io::Error {
    io::Error::other(s)
}
fn json_state(s: Lifecycle) -> &'static str {
    match s {
        Lifecycle::Idle => "idle",
        Lifecycle::Preparing => "preparing",
        Lifecycle::Ready => "ready",
        Lifecycle::Running => "running",
        Lifecycle::Degraded => "degraded",
        Lifecycle::Stopping => "stopping",
        Lifecycle::Failed => "failed",
    }
}
fn rand_hex() -> io::Result<String> {
    let mut bytes = [0u8; 16];
    let mut f = fs::File::open("/dev/urandom")?;
    f.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|x| format!("{x:02x}")).collect::<String>())
}
fn merge_settings(document: &mut Value, patch: &Value) -> io::Result<()> {
    let patch = patch
        .as_object()
        .ok_or_else(|| err("settings object required"))?;
    for (section, fields) in patch {
        let fields = fields
            .as_object()
            .ok_or_else(|| err("settings section object required"))?;
        if document.get(section).is_none() {
            document[section] = json!({});
        }
        let target = document[section]
            .as_object_mut()
            .ok_or_else(|| err("settings section object required"))?;
        for (key, value) in fields {
            target.insert(key.clone(), value.clone());
        }
    }
    Ok(())
}
fn apply_settings(cfg: &config::Config, patch: &Value) -> io::Result<config::Config> {
    let mut document: Value =
        serde_json::from_str(&cfg.document).map_err(|_| err("invalid normalized configuration"))?;
    for (section, fields) in patch
        .as_object()
        .ok_or_else(|| err("settings object required"))?
    {
        let allowed: &[&str] = match section.as_str() {
            "peer" => &["ssh_host"],
            "capture" => &["target", "game_audio"],
            "audio" => &["mic_source", "tts_sink", "mic_offset_ms"],
            "camera" => &[
                "enabled",
                "location",
                "device",
                "mode",
                "encoder",
                "hw_device",
                "width",
                "height",
                "fps",
                "offset_ms",
            ],
            "tts" => &["enabled", "mix_remote", "eventsub"],
            _ => return Err(err("unsupported selection section")),
        };
        if fields
            .as_object()
            .ok_or_else(|| err("settings section object required"))?
            .keys()
            .any(|key| !allowed.contains(&key.as_str()))
        {
            return Err(err("unsupported selection field"));
        }
    }
    merge_settings(&mut document, patch)?;
    let enabled = document["camera"]["enabled"].clone();
    let mix = document["tts"]["mix_remote"] == true;
    for route in document["routes"]
        .as_array_mut()
        .ok_or_else(|| err("routes missing"))?
    {
        if route["id"] == "camera" {
            route["enabled"] = enabled.clone();
        }
        if route["id"] == "tts" {
            route["targets"] = if mix {
                json!(["local.tts", "obs.tts"])
            } else {
                json!(["local.tts"])
            };
        }
    }
    config::Config::parse(document.to_string().as_bytes())
}
fn transport(cfg: &config::Config) -> io::Result<Value> {
    let document: Value =
        serde_json::from_str(&cfg.document).map_err(|_| err("invalid normalized configuration"))?;
    Ok(
        json!({"version":1,"video":{"codec":"H264","width":cfg.width,"height":cfg.height,"fps":cfg.fps},"audio":{"game":"MPEG-4 Audio","mic":"Opus","rate":48000},"ports":{"rtmp":cfg.rtmp,"rtsp":cfg.rtsp},"camera":document["camera"]}),
    )
}
fn require_rust_production(document: &str) -> io::Result<()> {
    let hello: Value = serde_json::from_str(document).map_err(|_| err("invalid peer status"))?;
    if hello["version"] != 1 || hello["role"] != "production" || hello["engine"] != "rust-candidate"
    {
        return Err(err(
            "the peer must be a compatible Rust production agent; Python delegation is forbidden",
        ));
    }
    Ok(())
}
pub fn doctor(cfg: &config::Config) -> io::Result<String> {
    let missing = media::check_tools(&cfg.role)?;
    let selected = cfg.role != "capture"
        || (!cfg.target.is_empty()
            && cfg.game_audio.starts_with("app:")
            && !cfg.mic_source.is_empty());
    Ok(json!({"ok":missing.is_empty()&&selected&&!cfg.eventsub_enabled&&!cfg.tts_enabled&&!cfg.tts_mix_remote,"engine":"rust-candidate","missing":missing,"capture_target_selected":!cfg.target.is_empty(),"game_only_audio_selected":cfg.game_audio.starts_with("app:"),"rnnoise_plugin_present":cfg.rnnoise_plugin.is_file(),"unimplemented":["EventSub","speech delivery and playback","cross-host shared selection transaction"],"competitive_non_regression":"NOT VERIFIED","cutover_authorized":false}).to_string())
}
pub fn cleanup_after_exit(cfg: config::Config) -> io::Result<()> {
    let agent = Agent::new(cfg)?;
    if agent.cfg.role == "production" {
        crate::audio::cleanup_socket(&agent.cfg)?;
    }
    Ok(())
}
struct Agent {
    operation: Option<crate::request::Operation>,
    cfg: config::Config,
    session: Session,
    children: Workers,
    error: String,
    warnings: Vec<String>,
    mic_muted: bool,
    last_heartbeat: Instant,
    _lock: fs::File,
    receive_monitor: Option<media::ReceiveMonitor>,
    last_health: Instant,
    output_observation: Option<obs::OutputStatus>,
}
impl Agent {
    fn new(mut cfg: config::Config) -> io::Result<Self> {
        let role = if cfg.role == "capture" {
            Role::Capture
        } else {
            Role::Production
        };
        cfg.state_dir = cfg.state_dir.join(&cfg.role);
        let directory = crate::state::private_directory(&cfg.state_dir)?;
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(
                std::path::PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()))
                    .join("agent.lock"),
            )?;
        let meta = lock.metadata()?;
        if !meta.is_file() || meta.uid() != unsafe { libc::getuid() } || meta.mode() & 0o077 != 0 {
            return Err(err("unsafe state lock file"));
        }
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(err("an agent already owns this role state directory"));
        }
        let saved = cfg.state_dir.join("settings.json");
        if saved.exists() {
            let patch: Value = serde_json::from_str(&config::secret(&saved, 65536)?)
                .map_err(|_| err("invalid saved selections"))?;
            let mut selected = apply_settings(&cfg, &patch)?;
            selected.state_dir = cfg.state_dir.clone();
            cfg = selected;
        }
        Ok(Self {
            operation: None,
            cfg,
            session: Session::new(role),
            children: Workers::default(),
            error: String::new(),
            warnings: vec![],
            mic_muted: false,
            last_heartbeat: Instant::now(),
            _lock: lock,
            receive_monitor: None,
            last_health: Instant::now(),
            output_observation: None,
        })
    }
    fn status(&self) -> String {
        let list = self
            .children
            .names()
            .iter()
            .map(|k| config::json_string(k))
            .collect::<Vec<_>>()
            .join(",");
        let notices = self
            .warnings
            .iter()
            .map(|x| config::json_string(x))
            .collect::<Vec<_>>()
            .join(",");
        let out = self
            .session
            .output()
            .map(|x| config::json_string(x.as_str()))
            .unwrap_or_else(|| "null".into());
        let completed_owner = self.session.control().completed_owner();
        let owner = completed_owner
            .as_ref()
            .map(|owner| config::json_string(owner.as_str()))
            .unwrap_or_else(|| "null".into());
        let stop = if self.session.control().stop_pending() {
            "\"requested\""
        } else if self.session.state() == Lifecycle::Stopping {
            "\"incomplete\""
        } else if self.session.control().stop_completed() {
            "\"completed\""
        } else {
            "null"
        };
        let outcome = match stop {
            "\"requested\"" => "\"accepted\"",
            "\"incomplete\"" => "\"unknown\"",
            "\"completed\"" => "\"completed\"",
            _ => "null",
        };
        format!("{{\"version\":1,\"engine\":\"rust-candidate\",\"role\":{},\"state\":{},\"output\":{out},\"error\":{},\"warnings\":[{notices}],\"mic_muted\":{},\"children\":[{list}],\"stop\":{stop},\"outcome\":{outcome},\"owner\":{owner},\"remote_release_pending\":{}}}",config::json_string(&self.cfg.role),config::json_string(json_state(self.session.state())),config::json_string(&self.error),self.mic_muted,self.session.remote_owner().is_some())
    }
    fn spawn(&mut self, worker: Worker, argv: Vec<String>) -> io::Result<()> {
        self.checkpoint()?;
        self.children.spawn(worker, &argv)
    }
    fn checkpoint(&self) -> io::Result<()> {
        self.operation
            .as_ref()
            .map_or(Ok(()), |operation| operation.check())
    }
    fn obs(&self) -> io::Result<obs::Obs> {
        self.checkpoint()?;
        obs::Obs::connect_operation(&self.cfg, self.operation.clone())
    }
    fn wait_ready<F>(&self, mut check: F) -> io::Result<()>
    where
        F: FnMut() -> bool,
    {
        let end = Instant::now() + Duration::from_secs(self.cfg.timeout as u64);
        while Instant::now() < end {
            self.checkpoint()?;
            if check() {
                self.checkpoint()?;
                return Ok(());
            }
            thread::sleep(Duration::from_millis(200));
        }
        Err(err("external service readiness timeout"))
    }
    fn rust_peer_ready(&self) -> bool {
        self.request_peer("status", "{}")
            .and_then(|s| require_rust_production(&s))
            .is_ok()
    }
    fn validate_peer_transport(&self) -> io::Result<()> {
        self.checkpoint()?;
        let response: Value = serde_json::from_str(&self.request_peer("routes", "{}")?)
            .map_err(|_| err("invalid peer route contract"))?;
        let document: Value = serde_json::from_str(&self.cfg.document)
            .map_err(|_| err("invalid local route contract"))?;
        if response["transport"] != transport(&self.cfg)?
            || response["routes"] != document["routes"]
        {
            return Err(err(
                "peer media and route contracts differ; no capture was started",
            ));
        }
        Ok(())
    }
    fn guard_lease(&self, owner: Option<&OwnerId>) -> io::Result<()> {
        self.session
            .authorize(owner)
            .map_err(|_| err("session is already owned; stop it locally before changing owners"))
    }
    fn renew_lease(&mut self, owner: &OwnerId) -> io::Result<()> {
        self.session
            .renew(owner, Instant::now(), self.lease_duration())
            .map_err(|_| err("no matching active production owner; prepare a new session"))
    }
    fn lease_duration(&self) -> Duration {
        Duration::from_secs((self.cfg.timeout as u64 * 8).max(30))
    }
    fn prepare(&mut self, owner: Option<&OwnerId>) -> io::Result<()> {
        self.checkpoint()?;
        self.guard_lease(owner)?;
        if matches!(self.session.state(), Lifecycle::Ready | Lifecycle::Running) {
            return Ok(());
        }
        if self.session.state() != Lifecycle::Idle {
            return Err(err("stop before preparing another session"));
        }
        self.session
            .prepare(owner.cloned(), Instant::now(), self.lease_duration())
            .map_err(|_| err("invalid prepare transition"))?;
        let outcome = (|| -> io::Result<()> {
            if self.cfg.eventsub_enabled || self.cfg.tts_enabled || self.cfg.tts_mix_remote {
                return Err(err("EventSub and speech delivery are not complete; they cannot be enabled in a prepared session"));
            }
            let missing = media::check_tools(&self.cfg.role)?;
            if !missing.is_empty() {
                return Err(err("SplitCast required native tools missing"));
            }
            if self.cfg.role == "capture" {
                self.spawn(Worker::Tunnel, media::tunnel(&self.cfg))?;
                self.wait_ready(|| self.rust_peer_ready())?;
                self.validate_peer_transport()?;
                self.prepare_remote(OwnerId::parse(&rand_hex()?)?)?;
            } else {
                self.spawn(Worker::Receiver, media::receiver(&self.cfg)?)?;
                self.wait_ready(|| {
                    media::receive_operation(&self.cfg, self.operation.as_ref()).is_ok()
                })?;
                self.spawn(Worker::AudioServer, crate::audio::server(&self.cfg)?)?;
                self.wait_ready(|| {
                    crate::audio::ready_operation(&self.cfg, self.operation.as_ref())
                        .unwrap_or(false)
                })?;
                self.spawn(
                    Worker::Obs,
                    crate::audio::client(&self.cfg, obs::launch_args(&self.cfg)?),
                )?;
                self.wait_ready(|| {
                    self.obs()
                        .and_then(|mut o| o.scene_ready())
                        .unwrap_or(false)
                })?;
                let mut o = self.obs()?;
                o.configure(&self.cfg)?;
            }
            Ok(())
        })();
        if let Err(e) = outcome {
            return Err(self.fail(e));
        }
        self.checkpoint().map_err(|e| self.fail(e))?;
        self.session.prepared().map_err(|_| err("prepare failed"))?;
        if let Some(o) = owner {
            self.renew_lease(o)?;
        }
        self.last_heartbeat = Instant::now();
        Ok(())
    }
    fn start(&mut self, which: &str, owner: Option<&OwnerId>) -> io::Result<()> {
        self.checkpoint()?;
        if which != "recording" && which != "twitch" {
            return Err(err("explicit recording/twitch output required"));
        }
        self.guard_lease(owner)?;
        if self.session.state() == Lifecycle::Running {
            return if self.session.output().map(Output::as_str) == Some(which) {
                Ok(())
            } else {
                Err(err("stop before changing outputs"))
            };
        }
        if self.session.state() == Lifecycle::Idle {
            if owner.is_some() {
                return Err(err(
                    "remote Start requires an existing matching preparation",
                ));
            }
            self.prepare(owner)?;
        }
        if self.session.state() != Lifecycle::Ready {
            return Err(err("not prepared"));
        }
        let outcome = (|| -> io::Result<()> {
            if self.cfg.role == "capture" {
                self.spawn(Worker::Game, media::game(&self.cfg)?)?;
                self.spawn(Worker::Microphone, media::mic(&self.cfg)?)?;
                if self.cfg.camera_enabled && self.cfg.camera_location == "capture" {
                    self.spawn(Worker::Camera, media::camera(&self.cfg)?)?;
                }
                self.remote_request(&Command::Start(if which == "recording" {
                    Output::Recording
                } else {
                    Output::Twitch
                }))?;
            } else {
                if self.cfg.camera_enabled && self.cfg.camera_location == "production" {
                    self.spawn(Worker::Camera, media::camera(&self.cfg)?)?;
                }
                let mut monitor = media::ReceiveMonitor::new(self.cfg.camera_enabled);
                self.wait_ready(|| {
                    media::receive_operation(&self.cfg, self.operation.as_ref())
                        .is_ok_and(|v| monitor.observe(&v, Instant::now()).unwrap_or(false))
                })?;
                self.spawn(
                    Worker::Rnnoise,
                    crate::audio::client(&self.cfg, media::rnnoise(&self.cfg)?),
                )?;
                let mut o = self.obs()?;
                o.configure(&self.cfg)?;
                self.wait_ready(|| {
                    self.obs()
                        .and_then(|mut o| o.game_decoded(&self.cfg))
                        .unwrap_or(false)
                })?;
                o.start(&self.cfg, which)?;
                self.wait_ready(|| {
                    media::receive_operation(&self.cfg, self.operation.as_ref())
                        .is_ok_and(|v| monitor.observe(&v, Instant::now()).unwrap_or(false))
                        && self
                            .obs()
                            .and_then(|mut o| o.observe(which))
                            .is_ok_and(|status| {
                                status.active
                                    && status.bytes.is_some_and(|bytes| bytes > 0)
                                    && status.frames.is_none_or(|frames| frames > 0)
                            })
                })?;
                self.receive_monitor = Some(monitor);
                self.output_observation = Some(self.obs()?.observe(which)?);
            }
            Ok(())
        })();
        if let Err(e) = outcome {
            return Err(self.fail(e));
        }
        let target = if which == "recording" {
            Output::Recording
        } else {
            Output::Twitch
        };
        self.checkpoint().map_err(|e| self.fail(e))?;
        self.session
            .start(target)
            .map_err(|_| err("start lifecycle error"))?;
        self.last_health = Instant::now();
        if let Some(o) = owner {
            self.renew_lease(o)?;
        }
        Ok(())
    }
    fn cleanup(&mut self) -> io::Result<()> {
        let previous = self.operation.replace(crate::request::Operation::new(
            Arc::new(AtomicBool::new(false)),
            None,
            Duration::from_secs(15),
        ));
        let result = self.cleanup_owned();
        self.operation = previous;
        result
    }
    fn cleanup_owned(&mut self) -> io::Result<()> {
        self.receive_monitor = None;
        self.output_observation = None;
        let mut failed = false;
        if self.cfg.role == "production" && self.children.contains(Worker::Obs) {
            failed |= self.obs().and_then(|mut o| o.stop()).is_err();
        }
        if self.session.remote_owner().is_some() {
            let release = (|| -> io::Result<()> {
                if !self.children.contains(Worker::Tunnel) {
                    self.spawn(Worker::Tunnel, media::tunnel(&self.cfg))?;
                    self.wait_ready(|| self.rust_peer_ready())?;
                    self.validate_peer_transport()?;
                }
                let mut response: Value =
                    serde_json::from_str(&self.remote_request(&Command::Stop)?)
                        .map_err(|_| err("invalid remote Stop completion"))?;
                loop {
                    if response["stop"] == "completed"
                        && response["state"] == "idle"
                        && response["owner"].as_str()
                            == self.session.remote_owner().as_ref().map(OwnerId::as_str)
                    {
                        break;
                    }
                    if response["stop"] != "requested" || response["outcome"] != "accepted" {
                        return Err(err(
                            "remote Stop did not prove completion for the owned session",
                        ));
                    }
                    // Observe the same accepted intent. Never retransmit Start
                    // or Prepare, or silently restart incomplete cleanup.
                    self.checkpoint()?;
                    thread::sleep(Duration::from_millis(20));
                    response = serde_json::from_str(&self.remote_request(&Command::Status)?)
                        .map_err(|_| err("invalid remote Stop observation"))?;
                }
                self.session
                    .remote_released()
                    .map_err(|_| err("invalid remote release transition"))
            })();
            if release.is_err() {
                failed = true;
                self.warn("remote stop unconfirmed; explicit Stop may retry and the production lease must expire");
            }
        }
        failed |= self.children.stop_all(Duration::from_millis(200)).is_err();
        if self.cfg.role == "production" && crate::audio::cleanup_socket(&self.cfg).is_err() {
            failed = true;
        }
        if failed {
            Err(err("owned resources could not all be released; stop again and inspect the enclosing service cgroup"))
        } else {
            Ok(())
        }
    }
    fn fail(&mut self, cause: io::Error) -> io::Error {
        if self.session.control().stop_pending() {
            // The admitted Stop owns compensation. Do not perform two cleanup
            // attempts or overwrite Stopping while unwinding cancelled work.
            let _ = self.session.stop();
            self.error = cause.to_string();
            return cause;
        }
        let _ = self.session.fail();
        let error = match self.cleanup() {
            Ok(()) => cause,
            Err(cleanup) => err(&format!("{cause}; cleanup incomplete: {cleanup}")),
        };
        self.error = error.to_string();
        error
    }
    fn stop(&mut self, owner: Option<&OwnerId>) -> io::Result<()> {
        if owner.is_some() {
            self.guard_lease(owner)?;
        }
        if self.session.state() == Lifecycle::Idle {
            return Ok(());
        }
        self.session
            .stop()
            .map_err(|_| err("stop lifecycle error"))?;
        if let Err(error) = self.cleanup() {
            self.error = error.to_string();
            return Err(error);
        }
        self.session
            .stopped()
            .map_err(|_| err("stop completion error"))?;
        self.error.clear();
        self.mic_muted = false;
        Ok(())
    }
    fn tick(&mut self) {
        self.expire_lease();
        if matches!(self.session.state(), Lifecycle::Ready | Lifecycle::Running) {
            if self.cfg.role == "capture"
                && self.children.contains(Worker::Tunnel)
                && self.last_heartbeat.elapsed() >= Duration::from_secs(2)
            {
                self.last_heartbeat = Instant::now();
                if self.remote_request(&Command::Heartbeat).is_err() {
                    let _ = self.fail(err("production heartbeat failed; owned capture stopped"));
                    return;
                }
            }
            let dead = self.children.any_exited();
            if dead {
                let _ = self.fail(err(
                    "owned infrastructure or media process exited; explicit prepare required",
                ));
            }
            if self.cfg.role == "production"
                && self.session.state() == Lifecycle::Running
                && self.last_health.elapsed() >= Duration::from_secs(1)
            {
                self.last_health = Instant::now();
                let health = (|| -> io::Result<bool> {
                    let snapshot = media::receive_operation(&self.cfg, self.operation.as_ref())?;
                    if !self
                        .receive_monitor
                        .as_mut()
                        .ok_or_else(|| err("receive monitor unavailable"))?
                        .observe(&snapshot, Instant::now())?
                    {
                        return Ok(false);
                    }
                    let output = self
                        .session
                        .output()
                        .ok_or_else(|| err("output unavailable"))?;
                    let current = self.obs()?.observe(output.as_str())?;
                    let progressing = self.output_observation.as_ref().is_some_and(|previous| {
                        current.active
                            && current
                                .bytes
                                .zip(previous.bytes)
                                .is_some_and(|(now, old)| now > old)
                            && current
                                .frames
                                .zip(previous.frames)
                                .is_none_or(|(now, old)| now > old)
                    });
                    self.output_observation = Some(current);
                    Ok(progressing)
                })();
                if !matches!(health, Ok(true)) {
                    let _ = self.fail(err("incoming media or OBS output stopped progressing; output stopped without automatic publication"));
                }
            }
        }
    }
    fn expire_lease(&mut self) {
        let expired = self.session.lease_expired(Instant::now());
        if expired {
            // Revoke queued work even when the owner beats the watchdog to
            // lease expiry and enters Stopping before the next listener poll.
            self.session.control().expire();
            if self.stop(None).is_ok() {
                self.warn("production owner lease expired; owned media stopped");
            } else {
                self.warn("production owner lease expired; cleanup is incomplete");
            }
        }
    }
    fn warn(&mut self, message: &str) {
        if self.warnings.len() == 16 {
            self.warnings.remove(0);
        }
        self.warnings.push(message.into());
    }
    #[cfg(test)]
    fn command(&mut self, cmd: &str, args: &str) -> io::Result<String> {
        self.expire_lease();
        let args: Value = serde_json::from_str(args).map_err(|_| err("invalid command JSON"))?;
        let invocation = Invocation::parse(cmd, &args, self.cfg.tts_max_text as usize)?;
        self.execute(invocation)
    }
    fn execute(&mut self, invocation: Invocation) -> io::Result<String> {
        self.checkpoint()?;
        let owner = invocation.owner.as_ref();
        if owner.is_some() && self.cfg.role != "production" {
            return Err(err("only production accepts remote ownership"));
        }
        if owner.is_some() {
            self.guard_lease(owner)?;
        }
        match invocation.command {
            Command::Status=>Ok(self.status()),
            Command::Heartbeat=>{self.renew_lease(owner.ok_or_else(||err("production owner required"))?)?;Ok(self.status())},
            Command::Doctor=>doctor(&self.cfg),
            Command::Prepare=>{self.prepare(owner)?;Ok(self.status())},
            Command::Start(output)=>{self.start(output.as_str(),owner)?;Ok(self.status())},
            Command::Stop=>{self.stop(owner)?;Ok(self.status())},
            Command::Diagnostics=>Ok(json!({"version":1,"role":self.cfg.role,"state":json_state(self.session.state()),"output":self.session.output().map(Output::as_str),"children":self.children.names(),"mic_muted":self.mic_muted,"retries":0}).to_string()),
            Command::Routes{settings}=>self.routes(settings.as_ref()),
            Command::Devices{remote}=>self.devices(remote),
            command=>{
                if !matches!(self.session.state(),Lifecycle::Ready|Lifecycle::Running|Lifecycle::Degraded){return Err(err("prepare the session first"));}
                if self.cfg.role=="capture"{return self.peer_command(&command);}
                if !self.children.contains(Worker::Obs){return Err(err("no owned OBS session"));}
                match command {
                    Command::Mic(action)=>{
                         let mut obs=self.obs()?;
                        if action!=MicAction::Status {
                            let muted=action==MicAction::Mute;
                            obs.call("SetInputMute",&json!({"inputName":"MicProcessed","inputMuted":muted}).to_string())?;self.mic_muted=muted;
                        }
                        let ret:Value=serde_json::from_str(&obs.call("GetInputMute","{\"inputName\":\"MicProcessed\"}")?).map_err(|_|err("invalid OBS mute response"))?;
                        let muted=ret["d"]["responseData"]["inputMuted"].as_bool().ok_or_else(||err("invalid OBS mute response"))?;
                        self.mic_muted=muted;Ok(json!({"muted":muted}).to_string())
                    },
                    Command::Overlay{text}=>{self.obs()?.overlay(&text)?;Ok(json!({"updated":true}).to_string())},
                    Command::Preview=>self.obs()?.preview(),
                    Command::Tts(_)|Command::Say{..}|Command::TtsNext|Command::TtsAck{..}|Command::TtsFinished{..}=>Err(err("speech synthesis is locally tested, but delivery, playback and EventSub parity are incomplete")),
                    _=>Err(err("command is not a prepared-session control"))
                }
            }
        }
    }
    fn execute_work(
        &mut self,
        work: crate::server::Work,
        shutdown: &Arc<AtomicBool>,
        limits: crate::server::Limits,
    ) {
        if shutdown.load(Ordering::Relaxed) {
            work.ticket.abandon();
            return;
        }
        if !work.ticket.begin() {
            return;
        }
        let control = self.session.control();
        let activation_owner = if self.cfg.role == "production"
            && matches!(
                work.invocation.command,
                Command::Prepare | Command::Start(_)
            ) {
            work.invocation.owner.clone()
        } else {
            None
        };
        if !control.begin(work.generation, activation_owner) {
            work.ticket.finish(crate::request::error_response(
                "not_executed",
                "request superseded by Stop",
            ));
            return;
        }
        self.operation = Some(
            crate::request::Operation::new(
                Arc::clone(shutdown),
                Some(Arc::clone(&work.ticket)),
                limits.operation,
            )
            .with_control(control.clone(), work.generation),
        );
        self.expire_lease();
        let response = self
            .execute(work.invocation)
            .map(|body| (200, body))
            .unwrap_or_else(|e| {
                (
                    409,
                    json!({"error":e.to_string(),"code":"command_failed","outcome":"unknown"})
                        .to_string(),
                )
            });
        control.end();
        work.ticket.finish(response);
    }
    fn prepare_remote(&mut self, owner: OwnerId) -> io::Result<String> {
        let cap = Duration::from_secs(self.cfg.timeout as u64);
        let operation = self
            .operation
            .clone()
            .unwrap_or_else(|| {
                crate::request::Operation::new(Arc::new(AtomicBool::new(false)), None, cap)
            })
            .with_remote_prepare(self.session.control(), owner.clone());
        let timeout = operation
            .deadline(cap)?
            .saturating_duration_since(Instant::now());
        control::request_operation(
            self.cfg.forward_control_port,
            self.cfg.remote_control_port,
            &self.cfg.token(true)?,
            "prepare",
            &json!({"_owner":owner.as_str()}).to_string(),
            timeout,
            Some(&operation),
        )
    }
    fn remote_request(&self, command: &Command) -> io::Result<String> {
        self.checkpoint()?;
        let owner = self
            .session
            .remote_owner()
            .ok_or_else(|| err("no owned remote preparation"))?;
        let (name, mut args) = command.request();
        args["_owner"] = json!(owner.as_str());
        self.request_peer(name, &args.to_string())
    }
    fn request_peer(&self, name: &str, args: &str) -> io::Result<String> {
        let cap = Duration::from_secs(self.cfg.timeout as u64);
        let timeout = self.operation.as_ref().map_or(Ok(cap), |op| {
            op.deadline(cap)
                .map(|end| end.saturating_duration_since(Instant::now()))
        })?;
        control::request_operation(
            self.cfg.forward_control_port,
            self.cfg.remote_control_port,
            &self.cfg.token(true)?,
            name,
            args,
            timeout,
            self.operation.as_ref(),
        )
    }
    fn peer_command(&mut self, command: &Command) -> io::Result<String> {
        let response = self.remote_request(command)?;
        if matches!(command, Command::Mic(_)) {
            let value: Value = serde_json::from_str(&response)
                .map_err(|_| err("invalid peer microphone response"))?;
            self.mic_muted = value["muted"]
                .as_bool()
                .ok_or_else(|| err("invalid peer microphone observation"))?;
        }
        Ok(response)
    }
    fn routes(&mut self, settings: Option<&Value>) -> io::Result<String> {
        if let Some(patch) = settings {
            if self.session.state() != Lifecycle::Idle {
                return Err(err("stop before changing routes"));
            }
            let mut cfg = apply_settings(&self.cfg, patch)?;
            let shared: serde_json::Map<String, Value> = patch
                .as_object()
                .unwrap()
                .iter()
                .filter(|(k, _)| matches!(k.as_str(), "camera" | "tts"))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            if self.cfg.role == "capture" && !shared.is_empty() {
                return Err(err("shared remote selections require a prepared compatible peer; no partial settings were saved"));
            }
            let saved = self.cfg.state_dir.join("settings.json");
            let mut previous: Value = if saved.exists() {
                serde_json::from_str(&config::secret(&saved, 65536)?)
                    .map_err(|_| err("invalid saved selections"))?
            } else {
                json!({})
            };
            merge_settings(&mut previous, patch)?;
            self.checkpoint()?;
            crate::state::atomic_file(&saved, previous.to_string())?;
            cfg.state_dir = self.cfg.state_dir.clone();
            self.cfg = cfg;
        }
        let doc: Value = serde_json::from_str(&self.cfg.document)
            .map_err(|_| err("invalid normalized configuration"))?;
        Ok(json!({"routes":doc["routes"],"transport":transport(&self.cfg)?,"selection":{"peer":{"ssh_host":self.cfg.ssh_host},"capture":{"target":self.cfg.target,"game_audio":self.cfg.game_audio},"audio":{"mic_source":self.cfg.mic_source,"tts_sink":self.cfg.tts_sink,"mic_offset_ms":self.cfg.mic_offset},"camera":doc["camera"],"tts":{"enabled":self.cfg.tts_enabled,"mix_remote":self.cfg.tts_mix_remote,"eventsub":self.cfg.eventsub_enabled}}}).to_string())
    }
    fn devices(&mut self, remote: bool) -> io::Result<String> {
        self.checkpoint()?;
        let local = media::devices_operation(&self.cfg.role, self.operation.as_ref())?;
        self.checkpoint()?;
        let mut result = json!({"local":local});
        if remote && self.cfg.role == "capture" {
            let mut temporary = if !self.children.contains(Worker::Tunnel) {
                Some(crate::OwnedProcess::spawn(&media::tunnel(&self.cfg))?)
            } else {
                None
            };
            let response = (|| {
                self.wait_ready(|| self.rust_peer_ready())?;
                self.request_peer("devices", "{\"remote\":false}")
            })();
            if let Some(tunnel) = &mut temporary {
                tunnel.stop(Duration::from_millis(100))?;
            }
            let remote: Value =
                serde_json::from_str(&response?).map_err(|_| err("invalid remote discovery"))?;
            result["remote"] = remote["local"].clone();
        }
        Ok(result.to_string())
    }
}
impl Drop for Agent {
    fn drop(&mut self) {
        let _ = self.stop(None);
    }
}
pub fn serve(cfg: config::Config, shutdown: Arc<AtomicBool>) -> io::Result<()> {
    let token = cfg.token(false)?;
    let listener = control::listener(&cfg)?;
    let directory = std::env::var_os("SPLITCAST_WEB_ASSETS").map(std::path::PathBuf::from);
    let assets = Arc::new(crate::web::Assets::load(directory.as_deref())?);
    eprintln!(
        "SplitCast Rust candidate: loopback control on 127.0.0.1:{}; no automatic streaming",
        cfg.control_port
    );
    serve_bound(
        cfg,
        listener,
        token,
        assets,
        shutdown,
        crate::server::Limits::default(),
    )
}
fn serve_bound(
    cfg: config::Config,
    listener: std::net::TcpListener,
    token: String,
    assets: Arc<crate::web::Assets>,
    shutdown: Arc<AtomicBool>,
    limits: crate::server::Limits,
) -> io::Result<()> {
    let mut agent = Agent::new(cfg)?;
    let (listener, pending) = crate::server::ListenerOwner::start(
        listener,
        token,
        agent.cfg.tts_max_text as usize,
        assets,
        Arc::clone(&shutdown),
        crate::server::Limits {
            lease: agent.lease_duration(),
            ..limits
        },
        agent.session.control(),
    )?;
    while !shutdown.load(Ordering::Relaxed) {
        let control = agent.session.control();
        if control.stop_pending() {
            let response = match agent.stop(None) {
                Ok(()) => {
                    let mut body: Value = serde_json::from_str(&agent.status()).expect("agent status is JSON");
                    body["stop"] = json!("completed");
                    body["outcome"] = json!("completed");
                    (200, body.to_string())
                }
                Err(error) => (409, json!({"error":error.to_string(),"state":json_state(agent.session.state()),"stop":"incomplete","outcome":"unknown","code":"cleanup_incomplete"}).to_string()),
            };
            control.finish_stop(response);
        }
        agent.operation = Some(
            crate::request::Operation::new(Arc::clone(&shutdown), None, limits.operation)
                .with_control(control.clone(), control.generation()),
        );
        agent.tick();
        match pending.recv_timeout(Duration::from_millis(20)) {
            Ok(work) => {
                agent.execute_work(work, &shutdown, limits);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    shutdown.store(true, Ordering::Relaxed);
    for work in pending.try_iter() {
        work.ticket.abandon();
    }
    let joined = listener.join();
    agent.operation = None;
    let cleaned = agent.stop(None);
    match (joined, cleaned) {
        (Err(join), Err(cleanup)) => Err(err(&format!("{join}; cleanup incomplete: {cleanup}"))),
        (Err(error), _) | (_, Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}
#[cfg(test)]
#[path = "agent_lifecycle_tests.rs"]
mod lifecycle_tests;
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    #[test]
    fn a_peer_status_is_not_proof_of_rust_ownership() {
        assert!(
            require_rust_production(r#"{"version":1,"role":"production","state":"ready"}"#)
                .is_err()
        );
        assert!(require_rust_production(
            r#"{"version":1,"role":"capture","engine":"rust-candidate"}"#
        )
        .is_err());
        require_rust_production(r#"{"version":1,"role":"production","engine":"rust-candidate"}"#)
            .unwrap();
    }

    fn peer_session(owner: &str, now: Instant, ttl: Duration) -> Session {
        let mut session = Session::new(Role::Production);
        let owner = OwnerId::parse(owner).unwrap();
        session
            .prepare(Some(owner.clone()), Instant::now(), ttl)
            .unwrap();
        session.prepared().unwrap();
        session.renew(&owner, now, ttl).unwrap();
        session
    }

    fn prepared_agent() -> Agent {
        let cfg = config::Config::load(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/agent-example.json"
        )))
        .unwrap();
        let mut cfg = cfg;
        cfg.role = "production".into();
        cfg.state_dir = std::env::temp_dir().join(format!(
            "splitcast-agent-test-{}-{}",
            std::process::id(),
            rand_hex().unwrap()
        ));
        let mut agent = Agent::new(cfg).unwrap();
        agent
            .session
            .prepare(None, Instant::now(), Duration::from_secs(30))
            .unwrap();
        agent.session.prepared().unwrap();
        agent
    }

    #[test]
    fn heartbeat_cannot_claim_a_locally_prepared_session() {
        let mut agent = prepared_agent();
        assert!(
            agent
                .command("heartbeat", "{\"_owner\":\"intruder\"}")
                .is_err(),
            "heartbeat claimed a locally owned session"
        );
    }

    #[test]
    fn matching_remote_owner_renews_without_starting_an_output() {
        let mut agent = prepared_agent();
        let deadline = Instant::now() + Duration::from_secs(1);
        agent.session = peer_session("session_123", Instant::now(), Duration::from_secs(1));
        agent
            .command("heartbeat", "{\"_owner\":\"session_123\"}")
            .unwrap();
        assert!(!agent
            .session
            .lease_expired(deadline + Duration::from_secs(1)));
        assert_eq!(agent.session.state(), Lifecycle::Ready);
        assert_eq!(agent.session.output(), None);
    }

    #[test]
    fn expired_owner_cannot_revive_its_lease() {
        let mut agent = prepared_agent();
        agent.session = peer_session(
            "expired",
            Instant::now() - Duration::from_secs(2),
            Duration::from_secs(1),
        );
        assert!(
            agent
                .command("heartbeat", "{\"_owner\":\"expired\"}")
                .is_err(),
            "expired owner revived its lease"
        );
        assert_eq!(agent.session.state(), Lifecycle::Idle);
    }

    #[test]
    fn owner_driven_expiry_fences_queued_prepare_before_the_watchdog_runs() {
        let mut agent = prepared_agent();
        agent.session = peer_session(
            "expired",
            Instant::now() - Duration::from_secs(2),
            Duration::from_secs(1),
        );
        let generation = agent.session.control().generation();
        let shutdown = Arc::new(AtomicBool::new(false));
        let queued: Vec<_> = [Command::Prepare, Command::Start(Output::Recording)]
            .into_iter()
            .map(|command| crate::server::Work {
                invocation: Invocation {
                    command,
                    owner: Some(OwnerId::parse("expired").unwrap()),
                },
                ticket: Arc::new(crate::request::RequestTicket::new(
                    Instant::now() + Duration::from_secs(5),
                )),
                generation,
            })
            .collect();
        // There is deliberately no listener/watchdog. This is the other expiry
        // ordering, which must revoke before stop changes Phase to Stopping.
        agent.expire_lease();
        for work in queued {
            let ticket = Arc::clone(&work.ticket);
            agent.execute_work(work, &shutdown, crate::server::Limits::default());
            let reply: Value = serde_json::from_str(&ticket.response().unwrap().1).unwrap();
            assert_eq!(reply["outcome"], "not_executed");
        }
        assert_eq!(agent.session.state(), Lifecycle::Idle);
        assert!(agent.children.names().is_empty());
        assert!(agent.session.output().is_none());
    }

    #[test]
    fn local_operator_can_emergency_stop_a_remote_session() {
        let mut agent = prepared_agent();
        agent.session = peer_session("remote", Instant::now(), Duration::from_secs(30));
        assert!(
            agent.command("stop", "{}").is_ok(),
            "local emergency stop was refused"
        );
        assert_eq!(agent.session.state(), Lifecycle::Idle);
    }

    #[test]
    fn repeated_start_is_idempotent_without_switching_to_twitch() {
        let mut agent = prepared_agent();
        agent.session.start(Output::Recording).unwrap();
        assert!(
            agent.start("recording", None).is_ok(),
            "repeated recording start was refused"
        );
        assert!(agent.start("twitch", None).is_err());
        assert_eq!(agent.session.output(), Some(Output::Recording));
    }

    #[test]
    fn command_rejects_unknown_arguments_and_non_string_owners() {
        let mut agent = prepared_agent();
        for (cmd, args) in [
            ("status", "{\"unexpected\":true}"),
            ("status", "{\"_owner\":[]}"),
            ("status", "null"),
            ("status", "{} {}"),
            ("start", "{\"output\":false}"),
        ] {
            assert!(agent.command(cmd, args).is_err(), "accepted {cmd} {args}");
        }
    }
}
