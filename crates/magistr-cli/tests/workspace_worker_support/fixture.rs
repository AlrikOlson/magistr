//! A dependency-free subprocess fixture, compiled by the integration harness.
//! Stdout is exclusively the real worker framing; diagnostics go to the log.

use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

fn record(log: &Path, event: &str) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .expect("fixture log");
    writeln!(file, "{} {event}", std::process::id()).expect("log event");
}

fn frame(bytes: &[u8]) {
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(&(bytes.len() as u32).to_le_bytes())
        .expect("frame header");
    stdout.write_all(bytes).expect("frame body");
    stdout.flush().expect("flush frame");
}

fn sleep_forever() -> ! {
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}

fn receive() -> Option<String> {
    let mut header = [0; 4];
    let mut stdin = std::io::stdin().lock();
    stdin.read_exact(&mut header).ok()?;
    let len = u32::from_le_bytes(header) as usize;
    assert!(len <= 32 * 1024 * 1024);
    let mut body = vec![0; len];
    stdin.read_exact(&mut body).ok()?;
    Some(String::from_utf8(body).expect("UTF-8 request"))
}

fn main() {
    let arguments: Vec<_> = std::env::args_os().collect();
    let mode = arguments[1].to_str().expect("fixture mode");
    if mode == "probe" {
        return;
    }
    let log = Path::new(&arguments[2]);
    if mode == "sleep-child" {
        record(log, "descendant");
        sleep_forever();
    }
    let previous_starts = std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter(|line| line.ends_with(" start"))
        .count();
    record(log, "start");
    if mode == "stall-start" {
        sleep_forever();
    }
    if mode == "wrong-version-ready" {
        frame(br#"{"v":2,"kind":"ready"}"#);
        sleep_forever();
    }
    frame(br#"{"v":1,"kind":"ready"}"#);
    while let Some(request) = receive() {
        if request.contains("\"shutdown\"") {
            record(log, "shutdown");
            return;
        }
        record(log, "request");
        match mode {
            "crash" => std::process::exit(42),
            "crash-once" if previous_starts == 0 => std::process::exit(42),
            "stall" => sleep_forever(),
            "stall-once" if previous_starts == 0 => sleep_forever(),
            "malformed-response" => {
                frame(b"{not-json");
                sleep_forever();
            }
            "oversize-response" => {
                std::io::stdout()
                    .write_all(&(32 * 1024 * 1024_u32 + 1).to_le_bytes())
                    .expect("oversize header");
                std::io::stdout().flush().expect("flush header");
                sleep_forever();
            }
            "wrong-id-response" | "wrong-version-response" => {
                let version = if mode == "wrong-version-response" {
                    2
                } else {
                    1
                };
                let id = if mode == "wrong-id-response" { 0 } else { 1 };
                let response = arguments[3].to_str().expect("response payload");
                frame(
                    format!(
                        r#"{{"v":{version},"kind":"response","id":{id},"response":{response}}}"#
                    )
                    .as_bytes(),
                );
                sleep_forever();
            }
            "descendant" => {
                let _child = std::process::Command::new(&arguments[0])
                    .args([std::ffi::OsStr::new("sleep-child"), log.as_os_str()])
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .expect("owned descendant");
                sleep_forever();
            }
            _ => {}
        }
        let id: String = request
            .split("\"id\":")
            .nth(1)
            .expect("request id")
            .trim_start()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        let response = arguments[3].to_str().expect("response payload");
        frame(format!(r#"{{"v":1,"kind":"response","id":{id},"response":{response}}}"#).as_bytes());
    }
}
