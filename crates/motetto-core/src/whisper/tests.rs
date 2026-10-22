use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
};

#[test]
fn parses_segments_with_absolute_offset_and_checks_bad_times() {
    let parts = parse(
        br#"{"segments":[{"start":0.25,"end":1.5,"text":" Hello "}]}"#,
        30000,
    )
    .unwrap();
    assert_eq!(
        parts,
        [Segment {
            start_ms: 30250,
            end_ms: 31500,
            text: "Hello".into()
        }]
    );
    for json in [
        r#"{"segments":[{"start":-1,"end":2,"text":"x"}]}"#,
        r#"{"segments":[{"start":2,"end":1,"text":"x"}]}"#,
        "{}",
        "invalid",
    ] {
        assert!(parse(json.as_bytes(), 0).is_err());
    }
    assert!(
        parse(
            br#"{"segments":[{"start":1,"end":2,"text":"x"}]}"#,
            u64::MAX
        )
        .is_err()
    );
}

fn serve(status: &str, body: &str) -> (u16, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let reply = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let handle = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        loop {
            let mut chunk = [0; 4096];
            let n = socket.read(&mut chunk).unwrap();
            if n == 0 {
                break;
            }
            bytes.extend_from_slice(&chunk[..n]);
            if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&bytes[..end]);
                let length: usize = header
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .map(str::to_owned)
                    })
                    .unwrap()
                    .parse()
                    .unwrap();
                if bytes.len() >= end + 4 + length {
                    break;
                }
            }
        }
        socket.write_all(reply.as_bytes()).unwrap();
        String::from_utf8_lossy(&bytes).into_owned()
    });
    (port, handle)
}

#[test]
fn multipart_posts_audio_and_does_not_skip_slice_offset() {
    let (port, server) = serve(
        "200 OK",
        r#"{"segments":[{"start":0,"end":1,"text":"hi"}]}"#,
    );
    let result = Client::new(port)
        .unwrap()
        .transcribe(&[0.0; 160], 30000, Some("en"), Some("Motetto"))
        .unwrap();
    assert_eq!(result[0].start_ms, 30000);
    let request = server.join().unwrap();
    assert!(request.starts_with("POST /inference HTTP/1.1"));
    for text in [
        "name=\"offset_t\"\r\n\r\n0",
        "name=\"prompt\"\r\n\r\nMotetto",
        "RIFF",
        "verbose_json",
    ] {
        assert!(request.contains(text));
    }
}

#[test]
fn rejects_redirects_server_errors_and_invalid_audio() {
    for status in [
        "302 Found\r\nLocation: http://192.0.2.1/inference",
        "500 Internal Server Error",
    ] {
        let (port, server) = serve(status, "");
        assert!(
            Client::new(port)
                .unwrap()
                .transcribe(&[0.0], 0, None, None)
                .is_err()
        );
        server.join().unwrap();
    }
    let client = Client::new(1).unwrap();
    assert!(client.transcribe(&[], 0, None, None).is_err());
    assert!(client.transcribe(&[f32::NAN], 0, None, None).is_err());
    assert!(Client::new(0).is_err());
}

#[test]
#[ignore = "requires local whisper-server and synthetic fixture; run scripts/m2-e2e.sh"]
fn real_sidecar_transcribes_synthetic_speech() {
    let port: u16 = std::env::var("MOTETTO_TEST_WHISPER_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let file =
        std::path::PathBuf::from(std::env::var("MOTETTO_TEST_MEDIA").unwrap()).join("wav.16k.wav");
    let samples = hound::WavReader::open(file)
        .unwrap()
        .samples::<f32>()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let segments = Client::new(port)
        .unwrap()
        .transcribe(&samples, 30000, Some("en"), None)
        .unwrap();
    assert!(
        segments
            .iter()
            .any(|s| s.text.to_ascii_lowercase().contains("friday"))
    );
    assert!(
        segments
            .iter()
            .all(|s| s.start_ms >= 30000 && s.end_ms >= s.start_ms)
    );
}
