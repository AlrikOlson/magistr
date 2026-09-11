//! A dependency-free LSP fixture launched by the real Wasmtime subprocess host.

use std::io::{BufRead, Read, Write};

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let log = std::path::Path::new(&args[1]);
    record(log, "start")?;
    let mut input = std::io::BufReader::new(std::io::stdin().lock());
    let mut output = std::io::stdout().lock();
    loop {
        let mut length = None;
        loop {
            let mut line = String::new();
            if input.read_line(&mut line)? == 0 {
                return Ok(());
            }
            if line == "\r\n" {
                break;
            }
            if let Some(value) = line.strip_prefix("Content-Length: ") {
                length = value.trim().parse::<usize>().ok();
            }
        }
        let length = length.expect("Content-Length provided by native host");
        assert!(length <= 4 * 1024 * 1024);
        let mut body = vec![0; length];
        input.read_exact(&mut body)?;
        let body = String::from_utf8(body).expect("guest request is JSON");
        record(log, "request")?;
        if args.get(2).is_some_and(|mode| mode == "stall") {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(60));
            }
        }
        let Some(id_start) = body.find("\"id\":") else {
            continue;
        };
        let id: String = body[id_start + 5..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        assert!(!id.is_empty(), "guest has a numeric request id");
        let location = r#"{"uri":"file:///workspace/lib.rs","range":{"start":{"line":4,"character":2},"end":{"line":4,"character":8}}}"#;
        let result = if body.contains("\"method\":\"initialize\"") {
            "{\"capabilities\":{}}".to_owned()
        } else if body.contains("\"method\":\"workspace/symbol\"") {
            format!("[{{\"name\":\"target\",\"kind\":12,\"location\":{location}}}]")
        } else if body.contains("\"method\":\"textDocument/references\"") {
            format!("[{location}]")
        } else {
            panic!("unexpected LSP request: {body}");
        };
        let response = format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{result}}}");
        write!(
            output,
            "Content-Length: {}\r\n\r\n{response}",
            response.len()
        )?;
        output.flush()?;
    }
}

fn record(path: &std::path::Path, event: &str) -> std::io::Result<()> {
    writeln!(
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?,
        "{} {event}",
        std::process::id()
    )
}
