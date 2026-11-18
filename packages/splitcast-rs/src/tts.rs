use crate::config::Config;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub(crate) const AUDIO_LIMIT: usize = 4 * 1024 * 1024;
const QUERY_LIMIT: usize = 256 * 1024;

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
fn origin(url: &str) -> io::Result<String> {
    let rest = url
        .strip_prefix("http://127.0.0.1:")
        .ok_or_else(|| invalid("VOICEVOX must use http://127.0.0.1:PORT"))?;
    let port = rest.strip_suffix('/').unwrap_or(rest);
    if port.is_empty()
        || !port.bytes().all(|b| b.is_ascii_digit())
        || port.parse::<u16>().ok().filter(|p| *p > 0).is_none()
    {
        return Err(invalid("invalid VOICEVOX port"));
    }
    Ok(format!("http://127.0.0.1:{port}"))
}
#[derive(Debug)]
pub(crate) struct PcmWav<'a> {
    pcm: &'a [u8],
    channels: u16,
    sample_rate: u32,
    duration: Duration,
}

impl<'a> PcmWav<'a> {
    pub(crate) fn parse(data: &'a [u8]) -> io::Result<Self> {
        if data.len() > AUDIO_LIMIT
            || data.len() < 12
            || &data[..4] != b"RIFF"
            || &data[8..12] != b"WAVE"
        {
            return Err(invalid("invalid WAV"));
        }
        let u32at = |i: usize| u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
        if (u32at(4) as usize).checked_add(8) != Some(data.len()) {
            return Err(invalid("invalid WAV length"));
        }
        let mut pos = 12;
        let mut fmt = None;
        let mut pcm = None;
        while pos + 8 <= data.len() {
            let len = u32at(pos + 4) as usize;
            let start = pos + 8;
            let end = start
                .checked_add(len)
                .filter(|end| *end <= data.len())
                .ok_or_else(|| invalid("truncated WAV"))?;
            if &data[pos..pos + 4] == b"fmt " {
                if fmt.is_some() || len < 16 {
                    return Err(invalid("duplicate or truncated WAV format"));
                }
                let u16at = |i: usize| u16::from_le_bytes([data[i], data[i + 1]]);
                fmt = Some((
                    u16at(start),
                    u16at(start + 2),
                    u32at(start + 4),
                    u32at(start + 8),
                    u16at(start + 12),
                    u16at(start + 14),
                ));
            } else if &data[pos..pos + 4] == b"data" {
                if fmt.is_none() {
                    return Err(invalid("WAV data before format"));
                }
                if pcm.is_some() {
                    return Err(invalid("duplicate WAV data"));
                }
                pcm = Some(&data[start..end]);
            }
            pos = end
                .checked_add(len % 2)
                .filter(|end| *end <= data.len())
                .ok_or_else(|| invalid("invalid WAV"))?;
        }
        if pos != data.len() {
            return Err(invalid("invalid WAV chunks"));
        }
        let (format, channels, rate, byte_rate, block_align, bits) =
            fmt.ok_or_else(|| invalid("missing WAV format"))?;
        let pcm = pcm.ok_or_else(|| invalid("missing WAV data"))?;
        let len = pcm.len();
        if format != 1
            || !(1..=2).contains(&channels)
            || !(8000..=48000).contains(&rate)
            || bits != 16
            || block_align != channels * 2
            || byte_rate != rate * u32::from(block_align)
            || len == 0
            || len % (channels as usize * 2) != 0
            || len / (channels as usize * 2) > rate as usize * 30
        {
            return Err(invalid("WAV must be bounded PCM16"));
        }
        let frames = len as u64 / u64::from(block_align);
        Ok(Self {
            pcm,
            channels,
            sample_rate: rate,
            duration: Duration::from_nanos((frames * 1_000_000_000).div_ceil(u64::from(rate))),
        })
    }

    pub(crate) fn pcm(&self) -> &'a [u8] {
        self.pcm
    }
    pub(crate) fn channels(&self) -> u16 {
        self.channels
    }
    pub(crate) fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    pub(crate) fn duration(&self) -> Duration {
        self.duration
    }
}
fn post(
    root: &str,
    path: &str,
    input: &[u8],
    cap: usize,
    cancel: &AtomicBool,
    deadline: Instant,
) -> io::Result<Vec<u8>> {
    check_cancel(cancel, deadline)?;
    let port = root
        .rsplit(':')
        .next()
        .unwrap()
        .parse::<u16>()
        .map_err(|_| invalid("invalid port"))?;
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(200))?;
    stream.set_read_timeout(Some(Duration::from_millis(100)))?;
    stream.set_write_timeout(Some(Duration::from_millis(100)))?;
    let request = format!("POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", input.len());
    let mut outgoing = request.into_bytes();
    outgoing.extend_from_slice(input);
    let mut written = 0;
    while written < outgoing.len() {
        check_cancel(cancel, deadline)?;
        match stream.write(&outgoing[written..]) {
            Ok(0) => return Err(io::Error::other("VOICEVOX connection closed")),
            Ok(n) => written += n,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) => {}
            Err(e) => return Err(e),
        }
    }
    let mut response = Vec::new();
    let mut framing = None;
    loop {
        check_cancel(cancel, deadline)?;
        let mut buf = [0; 8192];
        match stream.read(&mut buf) {
            Ok(0) => return Err(io::Error::other("truncated VOICEVOX response")),
            Ok(n) => response.extend_from_slice(&buf[..n]),
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) =>
            {
                continue
            }
            Err(e) => return Err(e),
        }
        if framing.is_none() {
            if let Some(end) = response.windows(4).position(|v| v == b"\r\n\r\n") {
                if end > 16384 {
                    return Err(invalid("HTTP headers too large"));
                }
                let headers = std::str::from_utf8(&response[..end])
                    .map_err(|_| invalid("invalid HTTP headers"))?;
                let mut lines = headers.split("\r\n");
                let status = lines.next().unwrap_or("");
                if !status.starts_with("HTTP/1.1 200 ") && !status.starts_with("HTTP/1.0 200 ") {
                    return Err(invalid("VOICEVOX HTTP error"));
                }
                let mut length = None;
                for line in lines {
                    let (key, value) = line
                        .split_once(':')
                        .ok_or_else(|| invalid("invalid HTTP header"))?;
                    if key.eq_ignore_ascii_case("transfer-encoding") {
                        return Err(invalid("unsupported HTTP framing"));
                    }
                    if key.eq_ignore_ascii_case("content-length") {
                        if length.is_some() {
                            return Err(invalid("duplicate HTTP length"));
                        }
                        length = Some(
                            value
                                .trim()
                                .parse::<usize>()
                                .map_err(|_| invalid("invalid HTTP length"))?,
                        );
                    }
                }
                let length = length.ok_or_else(|| invalid("missing HTTP length"))?;
                if length > cap {
                    return Err(invalid("VOICEVOX response too large"));
                }
                framing = Some((end + 4, length));
            } else if response.len() > 16384 {
                return Err(invalid("HTTP headers too large"));
            }
        }
        if let Some((start, length)) = framing {
            if response.len() >= start + length {
                return Ok(response[start..start + length].to_vec());
            }
        }
    }
}
fn check_cancel(cancel: &AtomicBool, deadline: Instant) -> io::Result<()> {
    if cancel.load(Ordering::Acquire) || Instant::now() >= deadline {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "VOICEVOX cancelled or timed out",
        ))
    } else {
        Ok(())
    }
}
fn synthesize(
    root: &str,
    speaker: u16,
    text: &str,
    cancel: &AtomicBool,
    expires: Instant,
) -> io::Result<Vec<u8>> {
    let deadline = expires.min(Instant::now() + Duration::from_secs(15));
    let query = post(
        root,
        &format!("/audio_query?text={}&speaker={speaker}", encode(text)),
        &[],
        QUERY_LIMIT,
        cancel,
        deadline,
    )?;
    let _: Value =
        serde_json::from_slice(&query).map_err(|_| invalid("invalid audio query JSON"))?;
    let audio = post(
        root,
        &format!("/synthesis?speaker={speaker}"),
        &query,
        AUDIO_LIMIT,
        cancel,
        deadline,
    )?;
    PcmWav::parse(&audio)?;
    check_cancel(cancel, deadline)?;
    Ok(audio)
}
struct Worker {
    job: Job,
    cancel: Arc<AtomicBool>,
    handle: JoinHandle<io::Result<(Vec<u8>, Duration)>>,
}

struct Job {
    id: String,
    text: String,
    expires: Instant,
    attempts: u8,
}

pub(crate) struct Speech {
    root: String,
    speaker: u16,
    max_text: usize,
    limit: usize,
    worker: Option<Worker>,
    pending: VecDeque<Job>,
    ready: VecDeque<(String, Instant, Vec<u8>)>,
    error: Option<&'static str>,
    last_synthesis_ms: Option<u128>,
}

impl Speech {
    pub(crate) fn new(cfg: &Config) -> io::Result<Self> {
        Ok(Self {
            root: origin(&cfg.voicevox_url)?,
            speaker: cfg.tts_speaker,
            max_text: cfg.tts_max_text.into(),
            limit: cfg.tts_queue_limit.into(),
            worker: None,
            pending: VecDeque::new(),
            ready: VecDeque::new(),
            error: None,
            last_synthesis_ms: None,
        })
    }

    fn expire(&mut self) {
        let now = Instant::now();
        self.pending.retain(|job| job.expires > now);
        self.ready.retain(|(_, expires, _)| *expires > now);
    }

    pub(crate) fn enqueue_until(
        &mut self,
        text: &str,
        id: &str,
        expires: Instant,
    ) -> io::Result<()> {
        self.expire();
        if expires <= Instant::now() || !safe_id(id) {
            return Err(invalid("invalid or expired speech admission"));
        }
        let text = text.trim();
        if text.is_empty()
            || text.chars().count() > self.max_text
            || text.chars().any(|c| c < ' ' && c != '\n')
        {
            return Err(invalid("invalid speech text"));
        }
        if self.pending.len() + self.ready.len() + usize::from(self.worker.is_some()) >= self.limit
        {
            return Err(invalid("TTS queue is full"));
        }
        self.pending.push_back(Job {
            id: id.into(),
            text: text.into(),
            expires,
            attempts: 0,
        });
        Ok(())
    }

    pub(crate) fn tick(&mut self) {
        self.expire();
        if let Some(worker) = &self.worker {
            if worker.job.expires <= Instant::now() {
                worker.cancel.store(true, Ordering::Release);
            }
            if !worker.handle.is_finished() {
                return;
            }
            let Worker { job, handle, .. } = self.worker.take().expect("finished worker is owned");
            let result = handle.join();
            if job.expires <= Instant::now() {
                self.error = Some("VOICEVOX synthesis expired");
            } else if let Ok(Ok((wav, elapsed))) = result {
                self.ready.push_back((job.id, job.expires, wav));
                self.last_synthesis_ms = Some(elapsed.as_millis());
                self.error = None;
            } else {
                self.error = Some("VOICEVOX synthesis failed");
                if job.attempts < 3 {
                    self.pending.push_front(job);
                }
            }
        }
        let Some(mut job) = self.pending.pop_front() else {
            return;
        };
        job.attempts += 1;
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let root = self.root.clone();
        let speaker = self.speaker;
        let text = job.text.clone();
        let expires = job.expires;
        match thread::Builder::new()
            .name("splitcast-voicevox".into())
            .spawn(move || {
                let started = Instant::now();
                synthesize(&root, speaker, &text, &flag, expires)
                    .map(|wav| (wav, started.elapsed()))
            }) {
            Ok(handle) => {
                self.worker = Some(Worker {
                    job,
                    cancel,
                    handle,
                })
            }
            Err(_) => self.error = Some("could not start VOICEVOX worker"),
        }
    }

    /// Consume before constructing a reply. A lost reply deliberately loses the utterance.
    pub(crate) fn take_return(&mut self) -> Option<(String, Instant, Vec<u8>)> {
        self.expire();
        self.ready.pop_front()
    }

    pub(crate) fn status(&mut self) -> Value {
        self.expire();
        json!({
            "enabled":true,
            "pending":self.pending.len(),
            "ready":self.ready.len(),
            "active":self.worker.is_some(),
            "error":self.error,
            "last_synthesis_ms":self.last_synthesis_ms
        })
    }
}

impl Drop for Speech {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            worker.cancel.store(true, Ordering::Release);
            let _ = worker.handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    #[test]
    fn tick_returns_while_voicevox_is_delayed() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let root = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(1);
            while Instant::now() < deadline {
                match listener.accept() {
                    Ok((_stream, _)) => {
                        thread::sleep(Duration::from_millis(500));
                        break;
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(e) => panic!("{e}"),
                }
            }
        });
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = thread::spawn(move || {
            let mut s = setup(root);
            enqueue(&mut s, "hello", "delayed").unwrap();
            s.tick();
            tx.send(()).unwrap();
        });
        let prompt = rx.recv_timeout(Duration::from_millis(200)).is_ok();
        server.join().unwrap();
        worker.join().unwrap();
        assert!(prompt, "tick blocked on VOICEVOX");
    }
    fn setup(root: String) -> Speech {
        Speech {
            root,
            speaker: 1,
            max_text: 50,
            limit: 2,
            worker: None,
            pending: VecDeque::new(),
            ready: VecDeque::new(),
            error: None,
            last_synthesis_ms: None,
        }
    }
    fn enqueue(speech: &mut Speech, text: &str, id: &str) -> io::Result<()> {
        speech.enqueue_until(text, id, Instant::now() + Duration::from_secs(5))
    }
    fn sample_wav() -> Vec<u8> {
        let mut v = b"RIFF".to_vec();
        v.extend(38u32.to_le_bytes());
        v.extend(b"WAVEfmt ");
        v.extend(16u32.to_le_bytes());
        v.extend(1u16.to_le_bytes());
        v.extend(1u16.to_le_bytes());
        v.extend(8000u32.to_le_bytes());
        v.extend(16000u32.to_le_bytes());
        v.extend(2u16.to_le_bytes());
        v.extend(16u16.to_le_bytes());
        v.extend(b"data");
        v.extend(2u32.to_le_bytes());
        v.extend([0, 0]);
        v
    }
    #[test]
    fn strict_wav_rejects_duplicate_chunks_and_inconsistent_rates() {
        let original = sample_wav();
        let mut cases = Vec::new();
        for (name, chunk) in [
            ("duplicate fmt", &original[12..36]),
            ("duplicate data", &original[36..]),
        ] {
            let mut bytes = original.clone();
            bytes.extend_from_slice(chunk);
            let length = (bytes.len() - 8) as u32;
            bytes[4..8].copy_from_slice(&length.to_le_bytes());
            cases.push((name, bytes));
        }
        let mut bytes = original.clone();
        bytes[28..32].copy_from_slice(&1u32.to_le_bytes());
        cases.push(("inconsistent byte rate", bytes));
        let mut bytes = original;
        bytes[32..34].copy_from_slice(&1u16.to_le_bytes());
        cases.push(("inconsistent block alignment", bytes));
        let accepted: Vec<_> = cases
            .iter()
            .filter_map(|(name, bytes)| PcmWav::parse(bytes).is_ok().then_some(*name))
            .collect();
        assert!(accepted.is_empty(), "invalid WAVs accepted: {accepted:?}");
    }
    #[test]
    fn strict_wav_bounds_frames_format_and_chunk_padding() {
        let original = sample_wav();
        let mut cases = Vec::new();
        for end in 0..original.len() {
            cases.push(original[..end].to_vec());
        }
        for (offset, value) in [(20, 3u16), (22, 0), (22, 3), (34, 8), (32, 0)] {
            let mut bytes = original.clone();
            bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
            cases.push(bytes);
        }
        for rate in [0u32, 7999, 48001, u32::MAX] {
            let mut bytes = original.clone();
            bytes[24..28].copy_from_slice(&rate.to_le_bytes());
            cases.push(bytes);
        }
        for length in [0u32, 1, 16000 * 30 + 2] {
            let mut bytes = original[..44].to_vec();
            bytes[40..44].copy_from_slice(&length.to_le_bytes());
            bytes.resize(44 + length as usize, 0);
            let total = (bytes.len() - 8) as u32;
            bytes[4..8].copy_from_slice(&total.to_le_bytes());
            cases.push(bytes);
        }
        let mut truncated_chunk = original.clone();
        truncated_chunk[40..44].copy_from_slice(&u32::MAX.to_le_bytes());
        cases.push(truncated_chunk);
        let mut missing_padding = original.clone();
        missing_padding.extend_from_slice(b"JUNK\x01\x00\x00\x00x");
        let length = (missing_padding.len() - 8) as u32;
        missing_padding[4..8].copy_from_slice(&length.to_le_bytes());
        cases.push(missing_padding);
        let mut leftover = original.clone();
        leftover.push(0);
        let length = (leftover.len() - 8) as u32;
        leftover[4..8].copy_from_slice(&length.to_le_bytes());
        cases.push(leftover);
        cases.push(vec![0; AUDIO_LIMIT + 1]);
        for (index, bytes) in cases.iter().enumerate() {
            assert!(
                PcmWav::parse(bytes).is_err(),
                "accepted invalid WAV case {index}"
            );
        }

        for channels in [1u16, 2] {
            for rate in [8000u32, 48000] {
                let mut bytes = original[..44].to_vec();
                let alignment = channels * 2;
                let byte_rate = rate * u32::from(alignment);
                // Both bounds apply: 30s at 48k stereo exceeds the 4 MiB cap.
                let seconds = 30.min((AUDIO_LIMIT as u32 - 54) / byte_rate);
                let length = byte_rate * seconds;
                bytes[22..24].copy_from_slice(&channels.to_le_bytes());
                bytes[24..28].copy_from_slice(&rate.to_le_bytes());
                bytes[28..32].copy_from_slice(&byte_rate.to_le_bytes());
                bytes[32..34].copy_from_slice(&alignment.to_le_bytes());
                bytes[40..44].copy_from_slice(&length.to_le_bytes());
                bytes.resize(44 + length as usize, 0);
                // Unknown chunks, including odd-length padded chunks, stay valid.
                bytes.extend_from_slice(b"JUNK\x01\x00\x00\x00x\x00");
                let total = (bytes.len() - 8) as u32;
                bytes[4..8].copy_from_slice(&total.to_le_bytes());
                let wav = PcmWav::parse(&bytes).unwrap();
                assert_eq!(wav.pcm().len(), length as usize);
                assert_eq!(wav.channels(), channels);
                assert_eq!(wav.sample_rate(), rate);
                assert_eq!(wav.duration(), Duration::from_secs(u64::from(seconds)));
            }
        }
    }
    #[test]
    fn rejects_redirects_and_oversized_http_bodies() {
        for response in [
            "HTTP/1.1 302 Found\r\nLocation: http://example.com/\r\nContent-Length: 0\r\n\r\n"
                .to_string(),
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                QUERY_LIMIT + 1
            ),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let root = format!("http://{}", listener.local_addr().unwrap());
            let server = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buf = [0; 4096];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(response.as_bytes());
            });
            assert!(post(
                &root,
                "/",
                &[],
                QUERY_LIMIT,
                &AtomicBool::new(false),
                Instant::now() + Duration::from_secs(2)
            )
            .is_err());
            server.join().unwrap();
        }
    }
    #[test]
    fn failed_requests_stop_after_three_attempts() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let root = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(2);
            let mut count = 0;
            while count < 3 && Instant::now() < deadline {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(Duration::from_secs(1)))
                            .unwrap();
                        let mut buf = [0; 4096];
                        let _ = stream.read(&mut buf);
                        let _ =
                            stream.write_all(b"HTTP/1.1 500 Error\r\nContent-Length: 0\r\n\r\n");
                        count += 1;
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(e) => panic!("{e}"),
                }
            }
            count
        });
        let mut s = setup(root);
        enqueue(&mut s, "hello", "retry").unwrap();
        s.tick();
        let deadline = Instant::now() + Duration::from_secs(2);
        while (s.worker.is_some() || !s.pending.is_empty()) && Instant::now() < deadline {
            s.tick();
            thread::sleep(Duration::from_millis(5));
        }
        assert!(s.worker.is_none() && s.pending.is_empty());
        assert!(s.take_return().is_none());
        assert_eq!(server.join().unwrap(), 3);
    }
    #[test]
    fn synthesis_and_consume_on_read() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let root = format!("http://{}", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let mut requests = Vec::new();
            for n in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut buf = [0u8; 4096];
                loop {
                    let count = stream.read(&mut buf).unwrap();
                    if count == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&buf[..count]);
                    if let Some(end) = bytes.windows(4).position(|x| x == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        let len = headers
                            .lines()
                            .find_map(|line| {
                                line.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .and_then(|v| v.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + len {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8_lossy(&bytes).into_owned());
                let body = if n == 0 {
                    b"{\"query\":true}".to_vec()
                } else {
                    sample_wav()
                };
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(&body).unwrap();
            }
            requests
        });
        let mut s = setup(root);
        enqueue(&mut s, "hello & 世界", "abc").unwrap();
        s.tick();
        let req = handle.join().unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while s.ready.is_empty() && Instant::now() < deadline {
            s.tick();
            thread::sleep(Duration::from_millis(5));
        }
        assert!(req[0]
            .starts_with("POST /audio_query?text=hello%20%26%20%E4%B8%96%E7%95%8C&speaker=1 "));
        assert!(req[1].ends_with("{\"query\":true}"));
        let (id, _, wav) = s.take_return().unwrap();
        assert_eq!(id, "abc");
        assert_eq!(wav, sample_wav());
        assert!(s.take_return().is_none());
    }

    #[test]
    fn queue_capacity_and_expiry() {
        let mut s = setup("http://127.0.0.1:1".into());
        enqueue(&mut s, "a", "a").unwrap();
        enqueue(&mut s, "b", "b").unwrap();
        assert!(enqueue(&mut s, "c", "c").is_err());
        s.pending.front_mut().unwrap().expires = Instant::now();
        assert_eq!(s.status()["pending"], 1);
        assert!(enqueue(&mut s, "d", "../bad").is_err());
    }
    #[test]
    fn rejects_non_loopback_and_invalid_wav() {
        for url in [
            "http://localhost:50021",
            "http://127.0.0.2:80",
            "http://127.0.0.1:1@evil",
            "http://127.0.0.1:1/path",
            "https://127.0.0.1:1",
            "http://127.0.0.1:0",
        ] {
            assert!(origin(url).is_err(), "{url}");
        }
        assert!(origin("http://127.0.0.1:50021/").is_ok());
        assert!(PcmWav::parse(&sample_wav()).is_ok());
        assert!(PcmWav::parse(b"RIFFxxxxWAVE").is_err());
    }
}
