//! `fakescan --headless`: the ready line, loopback only, and a clean exit when stdin closes.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn profiles() -> String {
    format!("{}/../../profiles", env!("CARGO_MANIFEST_DIR"))
}

/// `GET path` on `addr` (HTTP/1.0, so the server closes): the whole reply.
fn get(addr: &str, path: &str) -> String {
    let mut s = TcpStream::connect(addr).expect("connect");
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    write!(s, "GET {path} HTTP/1.0\r\nHost: {addr}\r\n\r\n").unwrap();
    let mut reply = String::new();
    s.read_to_string(&mut reply).expect("read");
    reply
}

#[test]
fn headless_prints_its_urls_listens_on_loopback_and_exits_when_stdin_closes() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fakescan"))
        .args([
            "--headless",
            "--exit-on-stdin-close",
            "--profiles",
            &profiles(),
            "--profile",
            "wia-lossless",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start fakescan");
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .expect("ready line");
    let json = line
        .trim()
        .strip_prefix("FAKESCAN_READY ")
        .unwrap_or_else(|| panic!("not a ready line: {line:?}"));
    let ready: serde_json::Value = serde_json::from_str(json).expect("ready JSON");

    let port = ready["port"].as_u64().expect("port");
    assert_ne!(port, 0, "the ephemeral port is the one bound");
    let base = ready["base_url"].as_str().unwrap();
    assert_eq!(base, format!("http://127.0.0.1:{port}"), "loopback only");
    assert_eq!(ready["scanner_url"], format!("{base}/ScannerService"));
    assert_eq!(ready["devices"][0]["profile"], "wia-lossless");

    let addr = base.strip_prefix("http://").unwrap();
    let health = get(addr, "/health");
    assert!(
        health.starts_with("HTTP/1.") && health.contains(" 200 "),
        "{health}"
    );

    drop(child.stdin.take());
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "{status}");
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "still running 20 s after stdin closed"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}
