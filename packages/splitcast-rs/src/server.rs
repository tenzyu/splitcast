use crate::{
    control,
    protocol::{Command, Invocation},
    request::{error_response, RequestTicket},
    web::Assets,
};
use std::io;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub(crate) struct Work {
    pub(crate) invocation: Invocation,
    pub(crate) ticket: Arc<RequestTicket>,
}
#[derive(Clone, Copy)]
pub(crate) struct Limits {
    pub(crate) queue: Duration,
    pub(crate) operation: Duration,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            queue: Duration::from_secs(5),
            operation: Duration::from_secs(120),
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
    assets: Arc<Assets>,
    limits: Limits,
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
        assets: Arc<Assets>,
        shutdown: Arc<AtomicBool>,
        limits: Limits,
    ) -> io::Result<(Self, mpsc::Receiver<Work>)> {
        let port = listener.local_addr()?.port();
        let config = Arc::new(ConnectionConfig {
            port,
            token,
            max_text,
            assets,
            limits,
        });
        let (send, receive) = mpsc::sync_channel(8);
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
                    connections.reap()?;
                    match listener.accept() {
                        Ok((stream, _)) => {
                            if connections.0.len() == 8 {
                                continue;
                            }
                            let socket = stream.try_clone()?;
                            let send = send.clone();
                            let config = Arc::clone(&config);
                            let cancel = Arc::clone(&cancel);
                            let thread = thread::Builder::new()
                                .name("splitcast-connection".into())
                                .spawn(move || {
                                    connection(stream, &config, send, cancel);
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
    mut stream: TcpStream,
    config: &ConnectionConfig,
    send: mpsc::SyncSender<Work>,
    cancel: Arc<AtomicBool>,
) {
    let limits = config.limits;
    let request = match control::recv(&mut stream, config.port, &config.token) {
        Ok(request) => request,
        Err(_) => {
            let _ = control::reject(&mut stream);
            return;
        }
    };
    if request.method == "GET" {
        if let Some((mime, body)) = config.assets.get(&request.path) {
            let _ = control::reply_bytes(&mut stream, 200, mime, body);
            return;
        }
        if request.path == "/" {
            let _ = control::reply(&mut stream, 503, "{\"error\":\"Rust WASM assets unavailable; use the TUI or build the web package\"}");
            return;
        }
    }
    let invocation = if request.method == "GET" && request.path == "/api/status" {
        Ok(Invocation {
            command: Command::Status,
            owner: None,
        })
    } else if request.method == "POST" && request.path == "/api/command" {
        Invocation::from_body(&request.body, config.max_text)
    } else {
        let _ = control::reply(&mut stream, 404, "{\"error\":\"unknown endpoint\"}");
        return;
    };
    let invocation = match invocation {
        Ok(invocation) => invocation,
        Err(e) => {
            let _ = control::reply(&mut stream, 409, &serde_json::json!({"error":e.to_string(),"code":"invalid_command","outcome":"not_executed"}).to_string());
            return;
        }
    };
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
    let _ = control::reply(&mut stream, response.0, &response.1);
}
