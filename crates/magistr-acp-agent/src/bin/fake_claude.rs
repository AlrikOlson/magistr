//! A fake `claude` that replays a recorded fixture, for the provider's tests.
//!
//! Usage: `fake_claude <fixture.ndjson> [the real flags, ignored]`. The
//! fixture is one of `tests/fixtures/claude/*.ndjson`: the CLI's frames in
//! order, interleaved with the driver's own `{"_sent": …}` markers. Those
//! markers are the fake's synchronisation points — it emits frames until it
//! reaches one, and continues once the matching input arrives on stdin:
//!
//! * `_sent` `user` — waits for a `user` message;
//! * `_sent` `control_request` — waits for a control request of that
//!   subtype, and maps its recorded request id to the live one so the
//!   recorded `control_response` that follows answers the live request;
//! * `_sent` `control_response` — waits for any control response (the
//!   provider's answer to a `can_use_tool`).
//!
//! Stdin closing ends the process, like the real thing.

use std::collections::HashMap;
use std::io::{BufRead, Write};

use serde_json::Value;

fn main() {
    let fixture = std::env::args()
        .nth(1)
        .expect("fake_claude <fixture.ndjson>");
    let text = std::fs::read_to_string(&fixture).expect("readable fixture");
    let frames: Vec<Value> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("fixture json"))
        .collect();
    let mut fake = Fake {
        frames,
        at: 0,
        ids: HashMap::new(),
        out: std::io::stdout(),
    };
    fake.emit(None);
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let Ok(input) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        fake.emit(Some(&input));
    }
}

struct Fake {
    frames: Vec<Value>,
    at: usize,
    /// Recorded request id → live request id.
    ids: HashMap<String, String>,
    out: std::io::Stdout,
}

impl Fake {
    /// Emit frames until a marker that `input` does not satisfy.
    fn emit(&mut self, input: Option<&Value>) {
        let mut input = input;
        while self.at < self.frames.len() {
            let frame = self.frames[self.at].clone();
            if let Some(sent) = frame.get("_sent") {
                let Some(live) = input.take() else { return };
                if !self.satisfies(sent, live) {
                    return;
                }
                self.at += 1;
                continue;
            }
            self.at += 1;
            let mut frame = frame;
            if frame["type"] == "control_response" {
                let recorded = frame["response"]["request_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                if let Some(live) = self.ids.get(&recorded) {
                    frame["response"]["request_id"] = Value::String(live.clone());
                }
            }
            let mut out = self.out.lock();
            let _ = writeln!(out, "{frame}");
            let _ = out.flush();
        }
    }

    /// Whether `live` is the input the recorded marker waited for; a control
    /// request also records the id mapping.
    fn satisfies(&mut self, sent: &Value, live: &Value) -> bool {
        match (sent["type"].as_str(), live["type"].as_str()) {
            (Some("user"), Some("user")) => true,
            (Some("control_request"), Some("control_request")) => {
                if sent["request"]["subtype"] != live["request"]["subtype"] {
                    return false;
                }
                if let (Some(recorded), Some(live_id)) =
                    (sent["request_id"].as_str(), live["request_id"].as_str())
                {
                    self.ids.insert(recorded.to_owned(), live_id.to_owned());
                }
                true
            }
            (Some("control_response"), Some("control_response")) => true,
            _ => false,
        }
    }
}
