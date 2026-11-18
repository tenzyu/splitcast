//! Production-only Twitch chat input. A prepared session owns the socket and
//! its bounded history and optional handoff to the existing speech queue.
use crate::chat::{Cursor, History};
use crate::{config, request::Operation};
use serde_json::{json, Value};
use std::collections::{HashSet, VecDeque};
use std::io::{self, Read, Write};
use std::net::{IpAddr, TcpStream};
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tungstenite::{
    client::IntoClientRequest, protocol::WebSocketConfig, stream::MaybeTlsStream, Message,
    WebSocket,
};

const HOST: &str = "eventsub.wss.twitch.tv";
const ENDPOINT: &str = "wss://eventsub.wss.twitch.tv/ws";
const SUBSCRIPTION: &str = "channel.chat.message";
const SCOPE: &str = "user:read:chat";
const HTTP_BUDGET: Duration = Duration::from_secs(4);
const VALIDATE_INTERVAL: Duration = Duration::from_secs(3600);
const FRAME_LIMIT: usize = 64 * 1024;
const MESSAGE_BUDGET: usize = 4096;
const REPLAY_WINDOW: Duration = Duration::from_secs(600);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

fn error(message: &str) -> io::Error {
    io::Error::other(message)
}

struct Credentials {
    token: String,
    client: String,
    broadcaster: String,
    reader: String,
}

impl Credentials {
    fn load(path: &Path) -> io::Result<Self> {
        let raw = config::secret(path, 8192)?;
        let json: Value =
            serde_json::from_str(&raw).map_err(|_| error("invalid Twitch credentials JSON"))?;
        let field = |name: &str, valid: fn(u8) -> bool, limit| {
            json[name]
                .as_str()
                .filter(|value| {
                    !value.is_empty() && value.len() <= limit && value.bytes().all(valid)
                })
                .map(str::to_owned)
                .ok_or_else(|| error("invalid Twitch credential field"))
        };
        Ok(Self {
            token: field("access_token", |b| b.is_ascii_alphanumeric(), 2048)?,
            client: field("client_id", |b| b.is_ascii_alphanumeric(), 128)?,
            broadcaster: field("broadcaster_user_id", |b| b.is_ascii_digit(), 32)?,
            reader: field("reader_user_id", |b| b.is_ascii_digit(), 32)?,
        })
    }

    fn condition(&self) -> Value {
        json!({"broadcaster_user_id":self.broadcaster, "user_id":self.reader})
    }

    fn validate(&self, http: &ureq::Agent) -> io::Result<()> {
        let response = http
            .get("https://id.twitch.tv/oauth2/validate")
            .header("Authorization", format!("OAuth {}", self.token))
            .call()
            .map_err(|_| error("Twitch OAuth validation request failed"))?;
        let data = read_json(response, 200)?;
        if data["client_id"] != self.client
            || data["user_id"] != self.reader
            || !data["scopes"]
                .as_array()
                .is_some_and(|scopes| scopes.iter().any(|scope| scope == SCOPE))
        {
            return Err(permission(
                "Twitch OAuth client, reading account or user:read:chat scope differs",
            ));
        }
        Ok(())
    }

    fn subscribe(&self, http: &ureq::Agent, session: &str) -> io::Result<String> {
        let body = json!({
            "type":SUBSCRIPTION, "version":"1", "condition":self.condition(),
            "transport":{"method":"websocket","session_id":session}
        });
        let response = http
            .post("https://api.twitch.tv/helix/eventsub/subscriptions")
            .header("Client-Id", &self.client)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/json")
            .send(body.to_string())
            .map_err(|_| error("Twitch chat subscription request failed"))?;
        let data = read_json(response, 202)?;
        let entry = &data["data"][0];
        if entry["type"] != SUBSCRIPTION
            || entry["version"] != "1"
            || entry["status"] != "enabled"
            || entry["condition"] != self.condition()
            || entry["transport"]["method"] != "websocket"
            || entry["transport"]["session_id"] != session
        {
            return Err(permission(
                "Twitch chat subscription response differs from requested identity",
            ));
        }
        Ok(identifier(&entry["id"])?.to_owned())
    }
}

fn read_json(mut response: ureq::http::Response<ureq::Body>, expected: u16) -> io::Result<Value> {
    if response.status() != expected {
        let code = response.status().as_u16();
        let detail = format!("Twitch HTTP status {code}");
        return Err(match code {
            400 | 401 | 403 => permission(&detail),
            _ => error(&detail),
        });
    }
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(FRAME_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| error("Twitch response body failed"))?;
    if bytes.len() > FRAME_LIMIT {
        return Err(error("Twitch response body too large"));
    }
    serde_json::from_slice(&bytes).map_err(|_| error("invalid Twitch response JSON"))
}

fn permission(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}

fn identifier(value: &Value) -> io::Result<&str> {
    value
        .as_str()
        .filter(|s| crate::protocol::valid_id(s))
        .ok_or_else(|| error("invalid Twitch identifier"))
}

pub(crate) struct Chat {
    pub(crate) received: Instant,
    pub(crate) text: String,
}

#[derive(Default)]
struct Receipt {
    state: &'static str,
    notifications: u64,
    duplicate: u64,
    dropped: u64,
    error: Option<String>,
    reconnects: u64,
}

// Kept across socket migrations and fresh subscriptions. Admission is at most
// once while the ID remains among the newest 4096 distinct messages and its
// first observation is under ten minutes old. A dropped handoff also consumes
// the ID. There is no backfill and no replay protection across Prepare sessions.
#[derive(Default)]
struct Replay {
    ids: HashSet<String>,
    order: VecDeque<(Instant, String)>,
}

impl Replay {
    fn admit(&mut self, id: &str, now: Instant) -> bool {
        while self
            .order
            .front()
            .is_some_and(|(at, _)| now.duration_since(*at) >= REPLAY_WINDOW)
        {
            self.evict();
        }
        if self.ids.contains(id) {
            return false;
        }
        if self.order.len() == MESSAGE_BUDGET {
            self.evict();
        }
        self.ids.insert(id.to_owned());
        self.order.push_back((now, id.to_owned()));
        true
    }

    fn evict(&mut self) {
        if let Some((_, id)) = self.order.pop_front() {
            self.ids.remove(&id);
        }
    }
}

pub(crate) struct EventSub {
    cancel: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    pub(crate) speech: Option<Receiver<Chat>>,
    receipt: Arc<Mutex<Receipt>>,
    history: Arc<Mutex<History>>,
}

// The session has one producer. Replay admission gates both independent sinks.
struct Feed {
    receipt: Arc<Mutex<Receipt>>,
    history: Arc<Mutex<History>>,
    speech: Option<SyncSender<Chat>>,
    replay: Replay,
}

impl Feed {
    fn notify(&mut self, id: &str, name: &str, text: &str, received: Instant) -> io::Result<()> {
        let mut status = self.receipt.lock().unwrap_or_else(|e| e.into_inner());
        status.notifications = status.notifications.saturating_add(1);
        if !self.replay.admit(id, received) {
            status.duplicate = status.duplicate.saturating_add(1);
            return Ok(());
        }
        self.history
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(id, name, text)?;
        if self.speech.as_ref().is_some_and(|send| {
            send.try_send(Chat {
                received,
                text: text.to_owned(),
            })
            .is_err()
        }) {
            status.dropped = status.dropped.saturating_add(1);
        }
        Ok(())
    }
}

impl EventSub {
    fn channel(epoch: String, queue: Option<usize>) -> (Self, Feed) {
        let (send, speech) = match queue {
            Some(queue) => {
                let (send, receive) = mpsc::sync_channel(queue);
                (Some(send), Some(receive))
            }
            None => (None, None),
        };
        let receipt = Arc::new(Mutex::new(Receipt {
            state: "connecting",
            ..Receipt::default()
        }));
        let history = Arc::new(Mutex::new(History::new(epoch)));
        let feed = Feed {
            receipt: Arc::clone(&receipt),
            history: Arc::clone(&history),
            speech: send,
            replay: Replay::default(),
        };
        (
            Self {
                cancel: Arc::new(AtomicBool::new(false)),
                worker: None,
                speech,
                receipt,
                history,
            },
            feed,
        )
    }

    pub(crate) fn start(path: &Path, epoch: String, queue: Option<usize>) -> io::Result<Self> {
        let creds = Credentials::load(path)?;
        let (mut subscription, feed) = Self::channel(epoch, queue);
        let cancel = Arc::clone(&subscription.cancel);
        subscription.worker = Some(
            thread::Builder::new()
                .name("splitcast-eventsub".into())
                .spawn(move || run(&creds, feed, &cancel))?,
        );
        Ok(subscription)
    }

    #[cfg(test)]
    pub(crate) fn synthetic(
        epoch: String,
        queue: Option<usize>,
    ) -> (Self, impl FnMut(&str, &str, &str, Instant)) {
        let (subscription, mut feed) = Self::channel(epoch, queue);
        (subscription, move |id, name, text, received| {
            feed.notify(id, name, text, received).unwrap()
        })
    }

    pub(crate) fn status(&self) -> Value {
        self.receipt
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .value()
    }

    pub(crate) fn page(&self, cursor: Option<&Cursor>) -> String {
        let status = self.receipt.lock().unwrap_or_else(|e| e.into_inner());
        let page = self
            .history
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .page(cursor, status.value());
        drop(status);
        page.to_string()
    }
}

impl Receipt {
    fn value(&self) -> Value {
        json!({
            "state":self.state,
            "notifications":self.notifications,
            "duplicates":self.duplicate,
            "dropped":self.dropped,
            "reconnects":self.reconnects,
            "error":self.error
        })
    }
}

impl Drop for EventSub {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        // Join the session owner. Network calls and the native DNS helper have
        // finite deadlines. The enclosing service cgroup remains the last resort.
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct DeadlineSocket {
    socket: TcpStream,
    cancel: Arc<AtomicBool>,
    end: Instant,
}

impl DeadlineSocket {
    fn io<T>(&mut self, mut act: impl FnMut(&mut TcpStream) -> io::Result<T>) -> io::Result<T> {
        loop {
            let remaining = self.end.saturating_duration_since(Instant::now());
            if remaining.is_zero() || self.cancel.load(Ordering::Relaxed) {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Twitch socket cancelled or expired",
                ));
            }
            let quantum = Some(remaining.min(Duration::from_millis(200)));
            self.socket.set_read_timeout(quantum)?;
            self.socket.set_write_timeout(quantum)?;
            match act(&mut self.socket) {
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock
                            | io::ErrorKind::TimedOut
                            | io::ErrorKind::Interrupted
                    ) => {}
                result => return result,
            }
        }
    }
}

impl Read for DeadlineSocket {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.io(|socket| socket.read(bytes))
    }
}

impl Write for DeadlineSocket {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.io(|socket| socket.write(bytes))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.io(Write::flush)
    }
}

type Socket = WebSocket<MaybeTlsStream<DeadlineSocket>>;

fn set_deadline(socket: &mut Socket, end: Instant) {
    if let MaybeTlsStream::Rustls(stream) = socket.get_mut() {
        stream.sock.end = end;
    }
}

fn connect(endpoint: &str, cancel: &Arc<AtomicBool>) -> io::Result<Socket> {
    let request = endpoint
        .into_client_request()
        .map_err(|_| error("invalid Twitch WebSocket URI"))?;
    let uri = request.uri();
    if uri.scheme_str() != Some("wss") || uri.authority().map(|a| a.as_str()) != Some(HOST) {
        return Err(error("Twitch reconnect URI has an unexpected origin"));
    }

    let operation = Operation::new(Arc::clone(cancel), None, HTTP_BUDGET);
    let result = crate::process::run_operation(
        "getent",
        &["ahosts".into(), HOST.into()],
        None,
        HTTP_BUDGET,
        16 * 1024,
        Some(&operation),
    )?;
    if !result.status.success() {
        return Err(error("Twitch WebSocket DNS failed"));
    }
    let output = String::from_utf8_lossy(&result.stdout);
    let mut tried = HashSet::new();
    let mut socket = None;
    for ip in output
        .lines()
        .filter_map(|line| line.split_whitespace().next()?.parse::<IpAddr>().ok())
    {
        if !tried.insert(ip) {
            continue;
        }
        let end = operation.deadline(Duration::from_millis(700))?;
        if let Ok(connected) = TcpStream::connect_timeout(
            &(ip, 443).into(),
            end.saturating_duration_since(Instant::now()),
        ) {
            socket = Some(connected);
            break;
        }
    }
    let socket = socket.ok_or_else(|| error("Twitch WebSocket TCP connection failed"))?;
    let stream = DeadlineSocket {
        socket,
        cancel: Arc::clone(cancel),
        end: operation.deadline(HTTP_BUDGET)?,
    };
    let settings = WebSocketConfig::default()
        .max_message_size(Some(FRAME_LIMIT))
        .max_frame_size(Some(FRAME_LIMIT));
    tungstenite::client_tls_with_config(request, stream, Some(settings), None)
        .map(|(ws, _)| ws)
        .map_err(|_| error("Twitch WebSocket TLS or upgrade failed"))
}

fn read(socket: &mut Socket) -> io::Result<Value> {
    loop {
        match socket
            .read()
            .map_err(|_| error("Twitch WebSocket read failed or keepalive expired"))?
        {
            Message::Text(text) => {
                return serde_json::from_str(&text)
                    .map_err(|_| error("invalid Twitch WebSocket JSON"))
            }
            Message::Ping(_) => socket
                .flush()
                .map_err(|_| error("Twitch WebSocket pong failed"))?,
            Message::Pong(_) => {}
            _ => {
                return Err(error(
                    "Twitch WebSocket closed or sent an unsupported message",
                ))
            }
        }
    }
}

fn kind(message: &Value) -> Option<&str> {
    message["metadata"]["message_type"].as_str()
}

fn welcome(socket: &mut Socket) -> io::Result<(String, Duration)> {
    set_deadline(socket, Instant::now() + HTTP_BUDGET);
    let message = read(socket)?;
    let session = &message["payload"]["session"];
    if kind(&message) != Some("session_welcome") || session["status"] != "connected" {
        return Err(error("Twitch WebSocket welcome required"));
    }
    let keepalive = session["keepalive_timeout_seconds"]
        .as_u64()
        .filter(|seconds| (1..=600).contains(seconds))
        .ok_or_else(|| error("invalid Twitch keepalive interval"))?;
    Ok((
        identifier(&session["id"])?.to_owned(),
        Duration::from_secs(keepalive),
    ))
}

fn chat<'a>(
    message: &'a Value,
    subscription: &str,
    creds: &Credentials,
) -> Option<(&'a str, &'a str, &'a str)> {
    let metadata = &message["metadata"];
    let sub = &message["payload"]["subscription"];
    let event = &message["payload"]["event"];
    if kind(message) != Some("notification")
        || metadata["subscription_type"] != SUBSCRIPTION
        || metadata["subscription_version"] != "1"
        || sub["id"] != subscription
        || sub["type"] != SUBSCRIPTION
        || sub["version"] != "1"
        || sub["condition"] != creds.condition()
        || sub["transport"]["method"] != "websocket"
        || event["broadcaster_user_id"] != creds.broadcaster
        || (!event["source_broadcaster_user_id"].is_null()
            && event["source_broadcaster_user_id"] != creds.broadcaster)
    {
        return None;
    }
    Some((
        identifier(&event["message_id"]).ok()?,
        event["chatter_user_name"].as_str().unwrap_or(""),
        event["message"]["text"].as_str()?,
    ))
}

fn run(creds: &Credentials, mut feed: Feed, cancel: &Arc<AtomicBool>) {
    let http: ureq::Agent = ureq::Agent::config_builder()
        .proxy(None)
        .max_redirects(0)
        .max_response_header_size(20 * 1024)
        .max_idle_connections(0)
        .timeout_global(Some(HTTP_BUDGET))
        .http_status_as_error(false)
        .https_only(true)
        .build()
        .into();
    let receipt = Arc::clone(&feed.receipt);
    reconnect(
        cancel,
        &receipt,
        |subscribed| {
            creds.validate(&http)?;
            let mut validated = Instant::now();
            let mut socket = connect(ENDPOINT, cancel)?;
            let (session, mut keepalive) = welcome(&mut socket)?;
            let subscription = creds.subscribe(&http, &session)?;
            subscribed();
            let mut until = Instant::now() + keepalive;
            loop {
                if cancel.load(Ordering::Relaxed) {
                    return Ok(());
                }
                if validated.elapsed() >= VALIDATE_INTERVAL {
                    creds.validate(&http)?;
                    validated = Instant::now();
                }
                set_deadline(&mut socket, until);
                let message = read(&mut socket)?;
                let received = Instant::now();
                match kind(&message) {
                    Some("session_reconnect") => {
                        let uri = message["payload"]["session"]["reconnect_url"]
                            .as_str()
                            .ok_or_else(|| error("Twitch reconnect URI missing"))?;
                        let mut next = connect(uri, cancel)?;
                        (_, keepalive) = welcome(&mut next)?;
                        // Twitch carries the existing subscription across a migration.
                        socket = next;
                        until = Instant::now() + keepalive;
                    }
                    Some("session_keepalive") => until = received + keepalive,
                    Some("revocation")
                        if message["payload"]["subscription"]["id"] == subscription =>
                    {
                        return Err(permission("Twitch revoked the chat subscription"));
                    }
                    Some("notification") => {
                        until = received + keepalive;
                        if let Some((id, name, text)) = chat(&message, &subscription, creds) {
                            feed.notify(id, name, text, received)?;
                        }
                    }
                    _ => {}
                }
            }
        },
        wait_to_retry,
    );
}

fn wait_to_retry(cancel: &AtomicBool, delay: Duration) {
    let until = Instant::now() + delay;
    while !cancel.load(Ordering::Relaxed) {
        let remaining = until.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        thread::sleep(remaining.min(Duration::from_millis(100)));
    }
}

fn reconnect(
    cancel: &AtomicBool,
    receipt: &Mutex<Receipt>,
    mut session: impl FnMut(&mut dyn FnMut()) -> io::Result<()>,
    mut wait: impl FnMut(&AtomicBool, Duration),
) {
    let mut backoff = Duration::from_secs(1);
    while !cancel.load(Ordering::Relaxed) {
        let mut connected_at = None;
        let result = session(&mut || {
            connected_at = Some(Instant::now());
            let mut status = receipt.lock().unwrap_or_else(|e| e.into_inner());
            status.state = "subscribed";
            status.error = None;
        });
        let Err(error) = result else { return };
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        let permanent = error.kind() == io::ErrorKind::PermissionDenied;
        {
            let mut status = receipt.lock().unwrap_or_else(|e| e.into_inner());
            status.state = if permanent { "failed" } else { "reconnecting" };
            status.error = Some(error.to_string());
            if !permanent {
                status.reconnects = status.reconnects.saturating_add(1);
            }
        }
        if permanent {
            return;
        }
        if connected_at.is_some_and(|at| at.elapsed() >= MAX_BACKOFF) {
            backoff = Duration::from_secs(1);
        }
        wait(cancel, backoff);
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn studio_chat_fans_out_independently_of_speech_capacity_and_expiry() {
        let epoch = "12345678901234567890123456789012".to_owned();
        let (sub, mut deliver) = EventSub::synthetic(epoch, Some(1));
        let mut cfg = config::Config::parse(include_bytes!("../tests/agent-example.json")).unwrap();
        cfg.tts_expiry_seconds = 5;
        let mut source = crate::return_audio::Source::new(&cfg, "speech-epoch".into()).unwrap();
        let old = Instant::now() - Duration::from_secs(6);
        let first = "あ".repeat(180);
        deliver("one", "viewer", &first, old);
        deliver("two", "viewer", "independent", old);
        deliver("one", "viewer", "duplicate", old);
        source.tick(Some(&sub));
        let speech: Value = serde_json::from_str(&source.status(Some(&sub)).unwrap()).unwrap();
        assert_eq!(speech["eventsub"]["dropped"], 1);
        assert_eq!(speech["eventsub"]["duplicates"], 1);
        assert_eq!(speech["eventsub"]["rejected"], 1);
        assert_eq!(speech["eventsub"]["admitted"], 0);
        assert_eq!(source.take(), "null");
        let page: Value = serde_json::from_str(&sub.page(None)).unwrap();
        assert_eq!(page["messages"][0]["text"].as_str().unwrap().len(), 510);
        assert_eq!(page["messages"][1]["text"], "independent");
        assert_eq!(page["cursor"], 2);

        let now = Instant::now();
        deliver("three", "viewer", &first, now);
        let handed_off = sub.speech.as_ref().unwrap().try_recv().unwrap();
        assert_eq!(handed_off.text, first);
        assert_eq!(handed_off.received, now);
    }

    #[test]
    fn keeps_ingesting_after_capacity_and_bounds_replay_by_age_and_count() {
        let mut replay = Replay::default();
        let now = Instant::now();
        for index in 0..=MESSAGE_BUDGET * 2 {
            let id = index.to_string();
            assert!(replay.admit(&id, now));
            assert!(!replay.admit(&id, now));
        }
        assert_eq!(replay.ids.len(), MESSAGE_BUDGET);
        assert_eq!(replay.order.len(), MESSAGE_BUDGET);
        assert!(replay.admit("0", now), "evicted IDs can be admitted again");
        assert!(!replay.admit("0", now + REPLAY_WINDOW - Duration::from_millis(1)));
        assert!(replay.admit("0", now + REPLAY_WINDOW));
        assert_eq!(replay.ids.len(), 1);
    }

    #[test]
    fn transient_outages_reconnect_beyond_three_attempts_and_preserve_replay() {
        let cancel = AtomicBool::new(false);
        let receipt = Mutex::new(Receipt::default());
        let mut calls = 0;
        let mut waits = Vec::new();
        let mut replay = Replay::default();
        reconnect(
            &cancel,
            &receipt,
            |subscribed| {
                calls += 1;
                if calls == 1 || calls == 8 {
                    subscribed();
                    assert_eq!(
                        replay.admit("already-delivered", Instant::now()),
                        calls == 1
                    );
                }
                if calls == 9 {
                    return Err(permission("credentials rejected"));
                }
                Err(error("synthetic disconnect"))
            },
            |_, delay| waits.push(delay.as_secs()),
        );
        assert_eq!(waits, [1, 2, 4, 8, 16, 30, 30, 30]);
        let receipt = receipt.into_inner().unwrap();
        assert_eq!(receipt.state, "failed");
        assert_eq!(receipt.reconnects, 8);
    }

    #[test]
    fn cancellation_during_backoff_does_not_open_another_socket() {
        let cancel = AtomicBool::new(false);
        let receipt = Mutex::new(Receipt::default());
        let mut calls = 0;
        reconnect(
            &cancel,
            &receipt,
            |_| {
                calls += 1;
                Err(error("synthetic outage"))
            },
            |cancel, _| cancel.store(true, Ordering::Relaxed),
        );
        assert_eq!(calls, 1);
    }

    #[test]
    fn accepts_only_current_configured_chat_subscription() {
        let creds = Credentials {
            token: "unused".into(),
            client: "unused".into(),
            broadcaster: "123".into(),
            reader: "456".into(),
        };
        let input = json!({
            "metadata":{"message_type":"notification","subscription_type":SUBSCRIPTION,"subscription_version":"1"},
            "payload":{
                "subscription":{"id":"correct","type":SUBSCRIPTION,"version":"1","condition":creds.condition(),"transport":{"method":"websocket"}},
                "event":{"message_id":"message-1","broadcaster_user_id":"123","message":{"text":"こんにちは"}}
            }
        });
        assert_eq!(
            chat(&input, "correct", &creds),
            Some(("message-1", "", "こんにちは"))
        );
        assert!(chat(&input, "old", &creds).is_none());
        for (path, value) in [
            ("/metadata/subscription_version", json!("2")),
            ("/payload/subscription/condition/user_id", json!("789")),
            ("/payload/event/broadcaster_user_id", json!("456")),
            ("/payload/event/message_id", json!("bad id")),
        ] {
            let mut invalid = input.clone();
            *invalid.pointer_mut(path).unwrap() = value;
            assert!(chat(&invalid, "correct", &creds).is_none(), "{path}");
        }
        let mut shared = input;
        shared["payload"]["event"]["source_broadcaster_user_id"] = json!("999");
        assert!(chat(&shared, "correct", &creds).is_none());
    }
}
