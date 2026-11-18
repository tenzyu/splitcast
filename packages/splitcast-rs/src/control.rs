//! Finite HTTP messages on authenticated loopback connections.
use crate::config::{json_string, Config};
use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::time::{Duration, Instant};

const HEADER_LIMIT: usize = 20 * 1024;
const BODY_LIMIT: usize = 16 * 1024;
const RESPONSE_LIMIT: usize = 4 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

fn err(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, msg)
}

fn same(a: &str, b: &str) -> bool {
    let x = a.as_bytes();
    let y = b.as_bytes();
    let mut diff = x.len() ^ y.len();
    for i in 0..x.len().max(y.len()) {
        diff |= (x.get(i).copied().unwrap_or(0) ^ y.get(i).copied().unwrap_or(0)) as usize;
    }
    diff == 0
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "HTTP deadline exceeded"))
}

struct DeadlineWriter<'a> {
    stream: &'a mut TcpStream,
    deadline: Instant,
    operation: Option<&'a crate::request::Operation>,
}

impl Write for DeadlineWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(operation) = self.operation {
            return operation.write(self.stream, bytes, self.deadline);
        }
        self.stream
            .set_write_timeout(Some(remaining(self.deadline)?))?;
        self.stream.write(bytes).map_err(write_error)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream
            .set_write_timeout(Some(remaining(self.deadline)?))?;
        self.stream.flush().map_err(write_error)
    }
}

fn write_error(error: io::Error) -> io::Error {
    if matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    ) {
        io::Error::new(io::ErrorKind::TimedOut, "HTTP write deadline exceeded")
    } else {
        error
    }
}

#[derive(Debug)]
struct UnknownOutcome(io::Error);

impl std::fmt::Display for UnknownOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "control outcome unknown after transmission began; inspect status and use explicit Stop before retrying; {}", self.0)
    }
}

impl std::error::Error for UnknownOutcome {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

fn unknown_outcome(error: io::Error) -> io::Error {
    io::Error::new(error.kind(), UnknownOutcome(error))
}

fn read_until_header(
    stream: &mut TcpStream,
    deadline: Instant,
    operation: Option<&crate::request::Operation>,
) -> io::Result<(String, Vec<u8>)> {
    let mut packet = Vec::new();
    loop {
        stream.set_read_timeout(Some(remaining(deadline)?))?;
        let mut buf = [0; 1024];
        let n = match operation {
            Some(operation) => operation.read(stream, &mut buf, deadline)?,
            None => stream.read(&mut buf)?,
        };
        if n == 0 {
            return Err(err("premature HTTP EOF"));
        }
        packet.extend_from_slice(&buf[..n]);
        if let Some(end) = packet.windows(4).position(|b| b == b"\r\n\r\n") {
            if end > HEADER_LIMIT {
                return Err(err("HTTP headers oversized"));
            }
            let header = std::str::from_utf8(&packet[..end])
                .map_err(|_| err("invalid HTTP headers"))?
                .to_owned();
            return Ok((header, packet[end + 4..].to_vec()));
        }
        if packet.len() > HEADER_LIMIT {
            return Err(err("HTTP headers oversized"));
        }
    }
}

fn headers<'a>(lines: impl Iterator<Item = &'a str>) -> io::Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    for line in lines {
        let (key, value) = line
            .split_once(':')
            .ok_or_else(|| err("invalid HTTP header"))?;
        if key.is_empty()
            || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || value.bytes().any(|b| b < 32 && b != b'\t' || b == 127)
        {
            return Err(err("invalid HTTP header"));
        }
        if result
            .insert(key.to_ascii_lowercase(), value.trim().to_owned())
            .is_some()
        {
            return Err(err("duplicate HTTP header"));
        }
    }
    if result.contains_key("transfer-encoding") {
        return Err(err("transfer encoding forbidden"));
    }
    Ok(result)
}

fn read_body(
    stream: &mut TcpStream,
    mut body: Vec<u8>,
    count: usize,
    deadline: Instant,
    operation: Option<&crate::request::Operation>,
) -> io::Result<Vec<u8>> {
    if body.len() > count {
        return Err(err("HTTP body/length mismatch"));
    }
    while body.len() < count {
        stream.set_read_timeout(Some(remaining(deadline)?))?;
        let mut buf = [0; 4096];
        let size = buf.len().min(count - body.len());
        let n = match operation {
            Some(operation) => operation.read(stream, &mut buf[..size], deadline)?,
            None => stream.read(&mut buf[..size])?,
        };
        if n == 0 {
            return Err(err("short HTTP body"));
        }
        body.extend_from_slice(&buf[..n]);
    }
    Ok(body)
}

#[derive(Debug)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub body: String,
}

pub fn recv(stream: &mut TcpStream, port: u16, token: &str) -> io::Result<Request> {
    recv_until(stream, port, token, Instant::now() + REQUEST_TIMEOUT)
}

fn recv_until(
    stream: &mut TcpStream,
    port: u16,
    token: &str,
    deadline: Instant,
) -> io::Result<Request> {
    let (head, body) = read_until_header(stream, deadline, None)?;
    let mut lines = head.split("\r\n");
    let parts = lines.next().unwrap_or("").split(' ').collect::<Vec<_>>();
    if parts.len() != 3 || parts[2] != "HTTP/1.1" || !matches!(parts[0], "GET" | "POST") {
        return Err(err("finite GET or POST HTTP/1.1 required"));
    }
    let h = headers(lines)?;
    let host = h.get("host").ok_or_else(|| err("Host required"))?;
    if host != &format!("127.0.0.1:{port}") && host != &format!("localhost:{port}") {
        return Err(err("loopback Host required"));
    }
    let origin = format!("http://{host}");
    if h.get("origin").is_some_and(|o| o != &origin) {
        return Err(err("same-origin mismatch"));
    }
    if h.get("origin") != Some(&origin) {
        return Err(err("same-origin request required"));
    }
    let bearer = h
        .get("authorization")
        .ok_or_else(|| err("bearer token required"))?;
    if !same(bearer, &format!("Bearer {token}")) {
        return Err(err("invalid bearer token"));
    }
    let count = if parts[0] == "POST" {
        if h.get("content-type").map(String::as_str) != Some("application/json") {
            return Err(err("JSON required"));
        }
        let len = h
            .get("content-length")
            .ok_or_else(|| err("Content-Length required"))?;
        if len.is_empty() || !len.bytes().all(|b| b.is_ascii_digit()) {
            return Err(err("invalid Content-Length"));
        }
        let n = len
            .parse::<usize>()
            .map_err(|_| err("invalid Content-Length"))?;
        if n == 0 || n > BODY_LIMIT {
            return Err(err("body exceeds 16KiB"));
        }
        n
    } else {
        if h.get("content-length").is_some_and(|s| s != "0") {
            return Err(err("GET body forbidden"));
        }
        0
    };
    Ok(Request {
        method: parts[0].to_owned(),
        path: parts[1].to_owned(),
        body: String::from_utf8(read_body(stream, body, count, deadline, None)?)
            .map_err(|_| err("invalid UTF-8 body"))?,
    })
}

pub fn reply_bytes(stream: &mut TcpStream, status: u16, mime: &str, body: &[u8]) -> io::Result<()> {
    reply_bytes_until(stream, status, mime, body, Instant::now() + REQUEST_TIMEOUT)
}

fn reply_bytes_until(
    stream: &mut TcpStream,
    status: u16,
    mime: &str,
    body: &[u8],
    deadline: Instant,
) -> io::Result<()> {
    let mut stream = DeadlineWriter {
        stream,
        deadline,
        operation: None,
    };
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        403 => "Forbidden",
        404 => "Not Found",
        503 => "Service Unavailable",
        _ => "Conflict",
    };
    write!(stream, "HTTP/1.1 {status} {reason}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'\r\n\r\n", body.len())?;
    stream.write_all(body)?;
    stream.flush()
}

pub fn reply(stream: &mut TcpStream, status: u16, json: &str) -> io::Result<()> {
    reply_bytes(
        stream,
        status,
        "application/json; charset=utf-8",
        json.as_bytes(),
    )
}

pub fn reject(stream: &mut TcpStream) -> io::Result<()> {
    reply(
        stream,
        403,
        "{\"error\":\"unauthorized or malformed request\"}",
    )?;
    // RFC 9112 section 9.6. Closing with unread request data can reset the response.
    stream.shutdown(Shutdown::Write)?;
    let deadline = Instant::now() + Duration::from_millis(100);
    let mut budget = 64 * 1024;
    let mut bytes = [0; 4096];
    while budget > 0 {
        let Ok(timeout) = remaining(deadline) else {
            break;
        };
        stream.set_read_timeout(Some(timeout))?;
        let count = budget.min(bytes.len());
        match stream.read(&mut bytes[..count]) {
            Ok(0) | Err(_) => break,
            Ok(n) => budget -= n,
        }
    }
    Ok(())
}

pub fn listener(cfg: &Config) -> io::Result<TcpListener> {
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, cfg.control_port))?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

pub fn request(
    port: u16,
    host_port: u16,
    token: &str,
    command: &str,
    args: &str,
    timeout: Duration,
) -> io::Result<String> {
    request_operation(port, host_port, token, command, args, timeout, None)
}

pub(crate) fn request_operation(
    port: u16,
    host_port: u16,
    token: &str,
    command: &str,
    args: &str,
    timeout: Duration,
    operation: Option<&crate::request::Operation>,
) -> io::Result<String> {
    let deadline = Instant::now() + timeout;
    let mut stream = TcpStream::connect_timeout(
        &SocketAddrV4::new(Ipv4Addr::LOCALHOST, port).into(),
        remaining(deadline)?,
    )?;
    let body = format!("{{\"command\":{},\"args\":{}}}", json_string(command), args);
    if body.len() > BODY_LIMIT {
        return Err(err("command exceeds 16KiB"));
    }
    let host = format!("127.0.0.1:{host_port}");
    let mut writer = DeadlineWriter {
        stream: &mut stream,
        deadline,
        operation,
    };
    write!(writer, "POST /api/command HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{host}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).map_err(unknown_outcome)?;
    writer.flush().map_err(unknown_outcome)?;
    let (head, body) =
        read_until_header(&mut stream, deadline, operation).map_err(unknown_outcome)?;
    let mut lines = head.split("\r\n");
    let status = lines.next();
    if !matches!(
        status,
        Some(
            "HTTP/1.1 200 OK"
                | "HTTP/1.1 202 Accepted"
                | "HTTP/1.1 409 Conflict"
                | "HTTP/1.1 503 Service Unavailable"
        )
    ) {
        return Err(unknown_outcome(err(
            "unrecognized control response; no redirects permitted",
        )));
    }
    let h = headers(lines).map_err(unknown_outcome)?;
    let size = h
        .get("content-length")
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| {
            *n <= if command == "tts-next" {
                crate::return_audio::REPLY_LIMIT
            } else {
                RESPONSE_LIMIT
            }
        })
        .ok_or_else(|| unknown_outcome(err("bounded control response required")))?;
    let response = String::from_utf8(
        read_body(&mut stream, body, size, deadline, operation).map_err(unknown_outcome)?,
    )
    .map_err(|_| unknown_outcome(err("invalid control response")))?;
    if status == Some("HTTP/1.1 200 OK") {
        return Ok(response);
    }
    let document: serde_json::Value = serde_json::from_str(&response)
        .map_err(|_| unknown_outcome(err("invalid control error response")))?;
    if status == Some("HTTP/1.1 202 Accepted")
        && command == "stop"
        && document["stop"] == "requested"
        && document["outcome"] == "accepted"
        && matches!(document["cleanup"].as_str(), Some("pending" | "unknown"))
    {
        // CLI/TUI callers receive the acknowledged intent. Remote release still
        // requires the agent's separately checked owner-bound completion proof.
        return Ok(response);
    }
    if document["outcome"] == "not_executed" {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "control command not executed; no redirects permitted",
        ));
    }
    Err(unknown_outcome(err(
        "control command failed; no redirects permitted",
    )))
}

pub fn request_peer(cfg: &Config, command: &str, args: &str) -> io::Result<String> {
    request(
        cfg.forward_control_port,
        cfg.remote_control_port,
        &cfg.token(true)?,
        command,
        args,
        Duration::from_secs(cfg.timeout as u64),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn exchange(extra: &str, method: &str, path: &str, body: &str) -> io::Result<Request> {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let mut client = TcpStream::connect(l.local_addr().unwrap()).unwrap();
        let request =
            format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{extra}\r\n{body}");
        client.write_all(request.as_bytes()).unwrap();
        let (mut server, _) = l.accept().unwrap();
        recv_until(
            &mut server,
            port,
            "test-token",
            Instant::now() + Duration::from_millis(100),
        )
    }

    #[test]
    fn all_control_endpoints_require_loopback_origin_and_token() {
        assert!(exchange("", "GET", "/", "").is_err());
        assert!(exchange("", "GET", "/api/status", "").is_err());
        assert!(exchange("Origin: https://evil.invalid\r\n", "GET", "/", "").is_err());
        assert!(exchange("Host: evil.invalid\r\n", "GET", "/", "").is_err());
    }

    #[test]
    fn rejects_ambiguous_framing_even_duplicate_transfer_encoding() {
        for header in [
            "Transfer-Encoding: chunked\r\nTransfer-Encoding: chunked\r\n",
            "Content-Length: 0\r\nContent-Length: 0\r\n",
            "Content-Length: 1\r\n",
            "X-Test : value\r\n",
        ] {
            assert!(
                exchange(header, "GET", "/", "").is_err(),
                "accepted {header:?}"
            );
        }
        assert!(exchange("", "GET", "/", "unexpected").is_err());
    }

    #[test]
    fn slow_headers_have_one_absolute_deadline() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client = TcpStream::connect(l.local_addr().unwrap()).unwrap();
        let writer = thread::spawn(move || {
            for _ in 0..20 {
                if client.write_all(b"G").is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(15));
            }
        });
        let (mut server, _) = l.accept().unwrap();
        let start = Instant::now();
        assert!(recv_until(
            &mut server,
            1234,
            "secret",
            start + Duration::from_millis(80)
        )
        .is_err());
        assert!(start.elapsed() < Duration::from_millis(250));
        drop(server);
        writer.join().unwrap();
    }

    #[test]
    fn rejects_redirect_and_status_code_substrings() {
        for status in [
            "HTTP/1.1 302 200 Redirect",
            "HTTP/1.1 403 200 Forbidden",
            "HTTP/1.1 500 Internal Server Error",
        ] {
            let l = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = l.local_addr().unwrap().port();
            let response = status.to_owned();
            let server = thread::spawn(move || {
                let (mut s, _) = l.accept().unwrap();
                recv(&mut s, port, "secret").unwrap();
                write!(s, "{response}\r\nContent-Length: 2\r\n\r\n{{}}").unwrap();
            });
            let error =
                request(port, port, "secret", "status", "{}", Duration::from_secs(1)).unwrap_err();
            assert!(error.to_string().contains("outcome unknown"), "{error}");
            assert!(error.to_string().contains("no redirects permitted"));
            server.join().unwrap();
        }
    }

    #[test]
    fn compares_full_tokens() {
        assert!(same("Bearer secret", "Bearer secret"));
        assert!(!same("Bearer secret", "Bearer secretx"));
    }

    #[test]
    fn lost_response_is_unknown_not_a_cancelled_command() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = recv(&mut stream, port, "test-token").unwrap();
            assert!(request.body.contains("start"));
        });
        let error = request(
            port,
            port,
            "test-token",
            "start",
            "{}",
            Duration::from_secs(1),
        )
        .unwrap_err();
        assert!(error.to_string().contains("outcome unknown"), "{error}");
        assert!(std::error::Error::source(error.get_ref().unwrap()).is_some());
        server.join().unwrap();
    }

    #[test]
    fn only_a_proven_prestart_refusal_is_reported_not_executed() {
        for (outcome, not_executed) in [("not_executed", true), ("unknown", false), ("", false)] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let server = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                recv(&mut stream, port, "test-token").unwrap();
                reply(
                    &mut stream,
                    503,
                    &serde_json::json!({
                        "outcome": outcome,
                        "error": "untrusted-peer-message-never-echo"
                    })
                    .to_string(),
                )
                .unwrap();
            });
            let error = request(
                port,
                port,
                "test-token",
                "start",
                "{}",
                Duration::from_secs(1),
            )
            .unwrap_err();
            assert_eq!(error.kind() == io::ErrorKind::Interrupted, not_executed);
            assert_eq!(error.to_string().contains("outcome unknown"), !not_executed);
            assert!(!error.to_string().contains("untrusted-peer-message"));
            server.join().unwrap();
        }
    }

    #[test]
    fn accepted_stop_is_not_proof_of_remote_release() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            recv(&mut stream, port, "test-token").unwrap();
            reply(
                &mut stream,
                202,
                r#"{"stop":"requested","outcome":"accepted","cleanup":"pending"}"#,
            )
            .unwrap();
        });
        let response = request(
            port,
            port,
            "test-token",
            "stop",
            "{}",
            Duration::from_secs(1),
        )
        .unwrap();
        let response: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(response["stop"], "requested");
        assert_eq!(response["cleanup"], "pending");
        server.join().unwrap();
    }

    #[test]
    fn slow_response_consumer_cannot_renew_the_write_budget() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut server, _) = listener.accept().unwrap();
        let finished = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let done = std::sync::Arc::clone(&finished);
        let reader = thread::spawn(move || {
            client
                .set_read_timeout(Some(Duration::from_millis(100)))
                .unwrap();
            let end = Instant::now() + Duration::from_secs(2);
            let mut received = 0;
            let mut bytes = [0; 8192];
            while Instant::now() < end && !done.load(std::sync::atomic::Ordering::Relaxed) {
                match client.read(&mut bytes) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => received += n,
                }
                thread::sleep(Duration::from_millis(10));
            }
            received
        });
        let body = vec![0; 16 * 1024 * 1024];
        let start = Instant::now();
        let result = reply_bytes_until(
            &mut server,
            200,
            "application/octet-stream",
            &body,
            start + Duration::from_millis(200),
        );
        let elapsed = start.elapsed();
        finished.store(true, std::sync::atomic::Ordering::Relaxed);
        drop(server);
        let received = reader.join().unwrap();
        assert!(received > 0, "consumer must make intermittent progress");
        assert!(
            elapsed < Duration::from_millis(700),
            "write budget renewed for {elapsed:?}"
        );
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
    }
}
