use splitcast_rs_core::run_bounded;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "splitcast-synthetic-local-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_owned()).collect()
}

#[test]
#[ignore = "runs local FFmpeg and FFprobe with generated inputs only"]
fn generated_audio_and_video_record_probe_and_fully_decode() {
    let fixture = Fixture::new();
    let recording = fixture.0.join("synthetic.mkv");
    let mut args = strings(&[
        "-nostdin",
        "-v",
        "error",
        "-filter_threads",
        "1",
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=320x180:rate=15",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:sample_rate=48000",
        "-t",
        "2",
        "-map",
        "0:v:0",
        "-map",
        "1:a:0",
        "-c:v",
        "libx264",
        "-threads",
        "1",
        "-preset",
        "ultrafast",
        "-pix_fmt",
        "yuv420p",
        "-c:a",
        "aac",
        "-f",
        "matroska",
    ]);
    args.push(recording.to_str().unwrap().into());
    let output = run_bounded("ffmpeg", &args, None, Duration::from_secs(30), 1024 * 1024).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut args = strings(&[
        "-v",
        "error",
        "-count_packets",
        "-show_entries",
        "stream=codec_name,nb_read_packets:format=duration",
        "-of",
        "json",
    ]);
    args.push(recording.to_str().unwrap().into());
    let output = run_bounded("ffprobe", &args, None, Duration::from_secs(10), 1024 * 1024).unwrap();
    assert!(output.status.success());
    let probe: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let streams = probe["streams"].as_array().unwrap();
    assert_eq!(streams.len(), 2);
    assert!(streams
        .iter()
        .any(|stream| stream["codec_name"] == "h264" && stream["nb_read_packets"] == "30"));
    assert!(streams.iter().any(|stream| stream["codec_name"] == "aac"
        && stream["nb_read_packets"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap()
            > 0));
    let duration = probe["format"]["duration"]
        .as_str()
        .unwrap()
        .parse::<f64>()
        .unwrap();
    assert!((duration - 2.0).abs() < 0.1);
    let mut args = strings(&["-nostdin", "-v", "error", "-xerror", "-threads", "1", "-i"]);
    args.push(recording.to_str().unwrap().into());
    args.extend(strings(&[
        "-map", "0:v:0", "-map", "0:a:0", "-f", "null", "-",
    ]));
    let output = run_bounded("ffmpeg", &args, None, Duration::from_secs(10), 1024 * 1024).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
