use crate::{
    control,
    protocol::{Command, Invocation},
    request::{error_response, RequestTicket},
};
use std::io;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc, Arc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const CONNECTION_LIMIT: usize = 8;
const NORMAL_WAIT_LIMIT: usize = 6;
const NORMAL_QUEUE_LIMIT: usize = NORMAL_WAIT_LIMIT - 1;
const STOP_ACK_WAIT: Duration = Duration::from_millis(50);

pub(crate) struct Work {
    pub(crate) invocation: Invocation,
    pub(crate) ticket: Arc<RequestTicket>,
    pub(crate) generation: u64,
}
#[derive(Clone, Copy)]
pub(crate) struct Limits {
    pub(crate) queue: Duration,
    pub(crate) operation: Duration,
    pub(crate) lease: Duration,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            queue: Duration::from_secs(5),
            operation: Duration::from_secs(120),
            lease: Duration::from_secs(30),
        }
    }
}
struct Connection {
    socket: TcpStream,
    thread: JoinHandle<()>,
}
struct Connections(Vec<Connection>);
impl Connections {
    fn close(&mut self) -> io::Result<()> {
        for connection in &self.0 {
            let _ = connection.socket.shutdown(Shutdown::Both);
        }
        let mut panicked = false;
        for connection in self.0.drain(..) {
            panicked |= connection.thread.join().is_err();
        }
        if panicked {
            Err(io::Error::other("control connection panicked"))
        } else {
            Ok(())
        }
    }
    fn reap(&mut self) -> io::Result<()> {
        let mut panicked = false;
        let mut i = 0;
        while i < self.0.len() {
            if self.0[i].thread.is_finished() {
                panicked |= self.0.swap_remove(i).thread.join().is_err();
            } else {
                i += 1;
            }
        }
        if panicked {
            Err(io::Error::other("control connection panicked"))
        } else {
            Ok(())
        }
    }
}
impl Drop for Connections {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
struct ConnectionConfig {
    port: u16,
    token: String,
    max_text: usize,
    limits: Limits,
    control: crate::lifecycle::SessionControl,
    normal_waiters: AtomicUsize,
}
pub(crate) struct ListenerOwner {
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<io::Result<()>>>,
}
impl ListenerOwner {
    pub(crate) fn start(
        listener: TcpListener,
        token: String,
        max_text: usize,
        shutdown: Arc<AtomicBool>,
        limits: Limits,
        control: crate::lifecycle::SessionControl,
    ) -> io::Result<(Self, mpsc::Receiver<Work>)> {
        let port = listener.local_addr()?.port();
        let config = Arc::new(ConnectionConfig {
            port,
            token,
            max_text,
            limits,
            control,
            normal_waiters: AtomicUsize::new(0),
        });
        let (send, receive) = mpsc::sync_channel(NORMAL_QUEUE_LIMIT);
        let cancel = Arc::clone(&shutdown);
        let thread = thread::Builder::new()
            .name("splitcast-listener".into())
            .spawn(move || {
                struct CancelOnExit(Arc<AtomicBool>);
                impl Drop for CancelOnExit {
                    fn drop(&mut self) {
                        self.0.store(true, Ordering::Relaxed);
                    }
                }
                let mut connections = Connections(Vec::new());
                let _cancel = CancelOnExit(Arc::clone(&cancel));
                while !cancel.load(Ordering::Relaxed) {
                    config.control.expire();
                    connections.reap()?;
                    match listener.accept() {
                        Ok((stream, _)) => {
                            if connections.0.len() == CONNECTION_LIMIT {
                                continue;
                            }
                            let socket = stream.try_clone()?;
                            let send = send.clone();
                            let config = Arc::clone(&config);
                            let cancel = Arc::clone(&cancel);
                            let thread = thread::Builder::new()
                                .name("splitcast-connection".into())
                                .spawn(move || {
                                    let mut stream = stream;
                                    connection(&mut stream, &config, send, cancel);
                                    // The listener retains a shutdown clone.
                                    // Completed HTTP must not wait for its next
                                    // reap pass before the peer receives EOF.
                                    let _ = stream.shutdown(Shutdown::Both);
                                })?;
                            connections.0.push(Connection { socket, thread });
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10))
                        }
                        Err(e) => return Err(e),
                    }
                }
                connections.close()
            })?;
        Ok((
            Self {
                shutdown,
                thread: Some(thread),
            },
            receive,
        ))
    }
    pub(crate) fn join(mut self) -> io::Result<()> {
        self.shutdown.store(true, Ordering::Relaxed);
        match self
            .thread
            .take()
            .expect("listener owned until join")
            .join()
        {
            Ok(result) => result,
            Err(_) => Err(io::Error::other("control listener panicked")),
        }
    }
}
impl Drop for ListenerOwner {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn connection(
    stream: &mut TcpStream,
    config: &ConnectionConfig,
    send: mpsc::SyncSender<Work>,
    cancel: Arc<AtomicBool>,
) {
    let limits = config.limits;
    let request = match control::recv(stream, config.port, &config.token) {
        Ok(request) => request,
        Err(_) => {
            let _ = control::reject(stream);
            return;
        }
    };
    let invocation = if request.method == "GET" && request.path == "/api/status" {
        Ok(Invocation {
            command: Command::Status,
            owner: None,
        })
    } else if request.method == "POST" && request.path == "/api/command" {
        Invocation::from_body(&request.body, config.max_text)
    } else {
        let _ = control::reply(stream, 404, "{\"error\":\"unknown endpoint\"}");
        return;
    };
    let invocation = match invocation {
        Ok(invocation) => invocation,
        Err(e) => {
            let _ = control::reply(stream, 409, &serde_json::json!({"error":e.to_string(),"code":"invalid_command","outcome":"not_executed"}).to_string());
            return;
        }
    };
    // recv has already enforced token, Host, Origin, framing and body limits.
    // SessionControl authorizes against the same metadata as lifecycle commits.
    if matches!(invocation.command, Command::Stop) {
        let response = match config.control.request_stop(invocation.owner.as_ref()) {
            Err(error) => (409, serde_json::json!({"error":error.to_string(),"code":"ownership_conflict","outcome":"not_executed"}).to_string()),
            Ok(ticket) => {
                    // A command flag cannot describe tick, lease maintenance,
                    // or failure cleanup. Bound acknowledgment independently
                    // of every effect-owner activity, including compensation.
                    let end = Instant::now() + STOP_ACK_WAIT;
                    loop {
                        if let Some(response) = ticket.response() { break response; }
                        if cancel.load(Ordering::Relaxed) || Instant::now() >= end {
                            let cleanup = if cancel.load(Ordering::Relaxed) { "unknown" } else { "pending" };
                            break (202, serde_json::json!({"stop":"requested","outcome":"accepted","cleanup":cleanup}).to_string());
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
            }
        };
        let _ = control::reply(stream, response.0, &response.1);
        return;
    }
    if matches!(invocation.command, Command::Heartbeat) {
        let response = config.control.heartbeat(invocation.owner.as_ref(), limits.lease)
            .map(|body| (200, body))
            .unwrap_or_else(|error| (409, serde_json::json!({"error":error.to_string(),"code":"ownership_conflict","outcome":"not_executed"}).to_string()));
        let _ = control::reply(stream, response.0, &response.1);
        return;
    }
    if matches!(invocation.command, Command::Status) {
        match config.control.stop_observation(invocation.owner.as_ref()) {
            Ok(Some(body)) => {
                let _ = control::reply(stream, 200, &body);
                return;
            }
            Ok(None) => {}
            Err(error) => {
                let _ = control::reply(stream, 409, &serde_json::json!({"error":error.to_string(),"code":"ownership_conflict","outcome":"not_executed"}).to_string());
                return;
            }
        }
    }
    let generation = match config.control.admit(matches!(
        invocation.command,
        Command::Status | Command::Diagnostics
    )) {
        Ok(generation) => generation,
        Err(error) => {
            let response = error_response("not_executed", &error.to_string());
            let _ = control::reply(stream, response.0, &response.1);
            return;
        }
    };
    // Bound ordinary response waiters separately from parser/control slots.
    // Slow unauthenticated headers can still exhaust the finite total cap.
    if config.normal_waiters.fetch_add(1, Ordering::Relaxed) >= NORMAL_WAIT_LIMIT {
        config.normal_waiters.fetch_sub(1, Ordering::Relaxed);
        let response = error_response("not_executed", "ordinary command capacity unavailable");
        let _ = control::reply(stream, response.0, &response.1);
        return;
    }
    struct NormalSlot<'a>(&'a AtomicUsize);
    impl Drop for NormalSlot<'_> {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::Relaxed);
        }
    }
    let _slot = NormalSlot(&config.normal_waiters);
    let expires = Instant::now() + limits.queue;
    let ticket = Arc::new(RequestTicket::new(expires));
    struct Abandon(Arc<RequestTicket>);
    impl Drop for Abandon {
        fn drop(&mut self) {
            self.0.abandon();
        }
    }
    let _abandon = Abandon(Arc::clone(&ticket));
    let response = if send
        .try_send(Work {
            invocation,
            ticket: Arc::clone(&ticket),
            generation,
        })
        .is_err()
    {
        error_response("not_executed", "command queue unavailable")
    } else {
        loop {
            if let Some(response) = ticket.response() {
                break response;
            }
            if cancel.load(Ordering::Relaxed) || Instant::now() >= expires + limits.operation {
                break ticket.abandon();
            }
            if Instant::now() >= expires && ticket.cancel_queued() {
                break ticket.abandon();
            }
            thread::sleep(Duration::from_millis(10));
        }
    };
    let _ = control::reply(stream, response.0, &response.1);
}
