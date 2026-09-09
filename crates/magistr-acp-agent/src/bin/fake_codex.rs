//! Replay the checked-in Codex App Server recordings without login or a model.
//!
//! Usage: `fake_codex <recording.ndjson> [--input-log <path>] [app-server flags]`.
//! `_sent` markers synchronize the replay with real provider input. Request ids
//! are remapped, while thread/turn ids and approval decisions must match exactly.
//! The optional log records launch arguments and every received frame for tests
//! of model, effort, cwd and MCP forwarding. No recorded command is executed.
//!
//! Tests may copy a recording and insert `_fault` (`eof`, `death`, `malformed`)
//! or `_delay_ms` markers. Those markers are fault injection, not claimed wire
//! recordings. Input, fixture size, delay and process lifetime are all bounded.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, Read, Write};
use std::time::Duration;

use serde_json::{Value, json};

const MAX_BYTES: u64 = 8 * 1024 * 1024;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().ok_or("fake_codex needs a recording path")?;
    let mut text = String::new();
    File::open(path)?
        .take(MAX_BYTES + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > MAX_BYTES {
        return Err("recording exceeds replay byte limit".into());
    }
    let frames = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<Vec<Value>, _>>()?;
    let log = args
        .windows(2)
        .find(|pair| pair[0] == "--input-log")
        .map(|pair| File::create(&pair[1]))
        .transpose()?;
    // An incorrect protocol sequence must never leave a test subprocess alive.
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(20));
        std::process::exit(124);
    });
    let mut replay = Replay {
        frames,
        at: 0,
        ids: HashMap::new(),
        log,
    };
    replay.log(&json!({"_argv": args}))?;
    if !replay.emit()? {
        return Ok(());
    }
    let stdin = std::io::stdin();
    let mut input = stdin.lock().take(MAX_BYTES);
    let mut line = String::new();
    while input.read_line(&mut line)? != 0 {
        let frame: Value = serde_json::from_str(&line)?;
        replay.log(&frame)?;
        replay.accept(&frame)?;
        line.clear();
        if !replay.emit()? {
            break;
        }
    }
    Ok(())
}

struct Replay {
    frames: Vec<Value>,
    at: usize,
    /// Recorded client request id -> live client request id. Server request ids
    /// are deliberately separate and are never rewritten through this table.
    ids: HashMap<String, Value>,
    log: Option<File>,
}

impl Replay {
    fn log(&mut self, frame: &Value) -> std::io::Result<()> {
        if let Some(log) = &mut self.log {
            writeln!(log, "{frame}")?;
            log.flush()?;
        }
        Ok(())
    }

    fn accept(&mut self, input: &Value) -> Result<(), Box<dyn std::error::Error>> {
        let sent = self
            .frames
            .get(self.at)
            .and_then(|frame| frame.get("_sent"))
            .ok_or("unexpected provider input after recording ended")?;
        if sent.get("method") != input.get("method") {
            return Err(format!("expected {sent}, received {input}").into());
        }
        if sent.get("method").is_some() {
            for key in ["threadId", "turnId", "expectedTurnId"] {
                if let Some(expected) = sent["params"].get(key)
                    && input["params"].get(key) != Some(expected)
                {
                    return Err(
                        format!("incorrect {key}: expected {sent}, received {input}").into(),
                    );
                }
            }
            if let Some(recorded) = sent.get("id") {
                let live = input.get("id").ok_or("request has no id")?;
                self.ids.insert(recorded.to_string(), live.clone());
            } else if input.get("id").is_some() {
                return Err("notification unexpectedly has an id".into());
            }
        } else if let Some(error) = sent.get("error") {
            // Injected unsupported-request checks pin the response identity
            // and protocol error code without coupling to explanatory prose.
            if sent["id"] != input["id"]
                || error["code"] != input["error"]["code"]
                || input["error"]["message"].as_str().is_none_or(str::is_empty)
                || input.get("result").is_some()
            {
                return Err(format!("expected server error {sent}, received {input}").into());
            }
        } else if sent != input {
            // A provider answering the wrong request or selecting the wrong
            // approval decision must fail, rather than silently resume replay.
            return Err(format!("expected server response {sent}, received {input}").into());
        }
        self.at += 1;
        Ok(())
    }

    /// Write until the next synchronization marker. False means an injected
    /// clean EOF; normal fixture exhaustion holds stdin open like app-server.
    fn emit(&mut self) -> Result<bool, Box<dyn std::error::Error>> {
        while let Some(frame) = self.frames.get(self.at) {
            if frame.get("_sent").is_some() {
                break;
            }
            let mut frame = frame.clone();
            self.at += 1;
            if let Some(delay) = frame["_delay_ms"].as_u64() {
                if delay > 1000 {
                    return Err("replay delay exceeds one second".into());
                }
                std::thread::sleep(Duration::from_millis(delay));
                continue;
            }
            if let Some(fault) = frame["_fault"].as_str() {
                match fault {
                    "eof" => return Ok(false),
                    "death" => std::process::exit(17),
                    "malformed" => {
                        let mut out = std::io::stdout().lock();
                        writeln!(out, "this is not JSON")?;
                        out.flush()?;
                        continue;
                    }
                    _ => return Err(format!("unknown injected fault: {fault}").into()),
                }
            }
            if frame.get("method").is_none()
                && let Some(id) = frame.get("id")
                && let Some(live) = self.ids.get(&id.to_string())
            {
                frame["id"] = live.clone();
            }
            let mut out = std::io::stdout().lock();
            writeln!(out, "{frame}")?;
            out.flush()?;
        }
        Ok(true)
    }
}
