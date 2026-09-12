//! A fake HTTP/1.1 server for the API providers: answers each connection
//! with the next scripted reply, records what was asked, and — for a reply
//! that holds — keeps the stream open until the client hangs up, which is
//! how a cancel is observed from the server's side.

// Shared by test binaries that each use a subset of it.
#![allow(dead_code)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// How a request is answered.
#[derive(Clone, Debug)]
pub enum Reply {
    /// Close before the advertised content length, exercising a transport error.
    Truncated {
        content_type: &'static str,
        body: String,
    },
    /// A whole body at once.
    Body {
        status: u16,
        content_type: &'static str,
        body: String,
    },
    /// A chunked body, one HTTP chunk per part with a pause between; when
    /// `hold` is set the terminating chunk is never sent and the server
    /// waits for the client to close instead.
    Stream {
        content_type: &'static str,
        parts: Vec<String>,
        hold: bool,
    },
}

impl Reply {
    /// A 200 with `body`, whole.
    pub fn ok(content_type: &'static str, body: impl Into<String>) -> Self {
        Self::Body {
            status: 200,
            content_type,
            body: body.into(),
        }
    }

    /// A refusal.
    pub fn error(status: u16, body: impl Into<String>) -> Self {
        Self::Body {
            status,
            content_type: "application/json",
            body: body.into(),
        }
    }

    /// A streamed body split at every blank line (SSE) or newline (NDJSON)
    /// boundary the fixture has, so each event is its own HTTP chunk.
    pub fn stream(content_type: &'static str, fixture: &str) -> Self {
        Self::Stream {
            content_type,
            parts: split_events(fixture),
            hold: false,
        }
    }

    /// [`stream`](Self::stream), but held open after the last part.
    pub fn held(content_type: &'static str, fixture: &str) -> Self {
        Self::Stream {
            content_type,
            parts: split_events(fixture),
            hold: true,
        }
    }
}

/// SSE fixtures separate events with a blank line; NDJSON ones with a
/// newline. Either way each event keeps its own terminator.
fn split_events(fixture: &str) -> Vec<String> {
    if fixture.contains("\n\n") {
        fixture.split_inclusive("\n\n").map(str::to_owned).collect()
    } else {
        fixture.split_inclusive('\n').map(str::to_owned).collect()
    }
}

/// One request as the server saw it.
#[derive(Clone, Debug, Default)]
pub struct Recorded {
    /// `POST /v1/chat/completions` — the request line's method and target.
    pub method: String,
    pub target: String,
    /// Header names lower-cased.
    pub headers: Vec<(String, String)>,
    pub body: String,
    /// Set when the reply was held and the client closed the connection.
    pub closed_by_client: bool,
}

impl Recorded {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).expect("the request body is JSON")
    }
}

/// The running server.
pub struct FakeHttp {
    /// `http://127.0.0.1:<port>` — no trailing slash.
    pub url: String,
    /// Every request, in arrival order.
    pub requests: Arc<Mutex<Vec<Recorded>>>,
    task: tokio::task::JoinHandle<()>,
}

impl FakeHttp {
    /// Serve `replies`, one per connection in order; a connection past the
    /// last reply is answered 500.
    pub async fn serve(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind a loopback port");
        let url = format!("http://{}", listener.local_addr().expect("addr"));
        let requests: Arc<Mutex<Vec<Recorded>>> = Arc::default();
        let task = tokio::spawn({
            let requests = Arc::clone(&requests);
            async move {
                let mut replies = replies.into_iter();
                loop {
                    let Ok((stream, _)) = listener.accept().await else {
                        return;
                    };
                    let reply = replies
                        .next()
                        .unwrap_or_else(|| Reply::error(500, "no reply"));
                    let requests = Arc::clone(&requests);
                    tokio::spawn(handle(stream, reply, requests));
                }
            }
        });
        Self {
            url,
            requests,
            task,
        }
    }

    /// The requests so far.
    pub fn recorded(&self) -> Vec<Recorded> {
        self.requests.lock().expect("lock").clone()
    }

    /// Wait until `n` requests have arrived and the `n`th one has ended
    /// (its reply sent, or its held connection closed by the client).
    pub async fn wait_for_requests(&self, n: usize) {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if self.recorded().len() >= n {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("the requests arrived before the timeout");
    }

    /// Wait until the client closed the `n`th (1-based) held connection.
    pub async fn wait_for_close(&self, n: usize) {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if self
                    .recorded()
                    .get(n - 1)
                    .is_some_and(|r| r.closed_by_client)
                {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("the client closed the held connection before the timeout");
    }
}

impl Drop for FakeHttp {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn handle(mut stream: TcpStream, reply: Reply, requests: Arc<Mutex<Vec<Recorded>>>) {
    let Some(recorded) = read_request(&mut stream).await else {
        return;
    };
    let index = {
        let mut requests = requests.lock().expect("lock");
        requests.push(recorded);
        requests.len() - 1
    };
    match reply {
        Reply::Truncated { content_type, body } => {
            let head = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len() + 128
            );
            let _ = stream.write_all(head.as_bytes()).await;
            let _ = stream.write_all(body.as_bytes()).await;
            let _ = stream.shutdown().await;
        }
        Reply::Body {
            status,
            content_type,
            body,
        } => {
            let head = format!(
                "HTTP/1.1 {status} {}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                reason(status),
                body.len()
            );
            let _ = stream.write_all(head.as_bytes()).await;
            let _ = stream.write_all(body.as_bytes()).await;
            let _ = stream.shutdown().await;
        }
        Reply::Stream {
            content_type,
            parts,
            hold,
        } => {
            let head = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n"
            );
            let _ = stream.write_all(head.as_bytes()).await;
            for part in parts {
                let chunk = format!("{:x}\r\n{part}\r\n", part.len());
                if stream.write_all(chunk.as_bytes()).await.is_err() {
                    break;
                }
                let _ = stream.flush().await;
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            if hold {
                // Never terminate: wait for the peer to go away.
                let mut sink = [0u8; 256];
                loop {
                    match stream.read(&mut sink).await {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {}
                    }
                }
                if let Some(recorded) = requests.lock().expect("lock").get_mut(index) {
                    recorded.closed_by_client = true;
                }
            } else {
                let _ = stream.write_all(b"0\r\n\r\n").await;
                let _ = stream.shutdown().await;
            }
        }
    }
}

/// Read the request head and its `content-length` body.
async fn read_request(stream: &mut TcpStream) -> Option<Recorded> {
    let mut buffer = Vec::new();
    let mut byte = [0u8; 1024];
    let head_end = loop {
        if let Some(pos) = find(&buffer, b"\r\n\r\n") {
            break pos + 4;
        }
        let n = stream.read(&mut byte).await.ok()?;
        if n == 0 {
            return None;
        }
        buffer.extend_from_slice(&byte[..n]);
    };
    let head = String::from_utf8_lossy(&buffer[..head_end]).into_owned();
    let mut lines = head.lines();
    let request_line = lines.next()?;
    let mut words = request_line.split_whitespace();
    let method = words.next()?.to_owned();
    let target = words.next()?.to_owned();
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(n, v)| (n.trim().to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    let length: usize = headers
        .iter()
        .find(|(n, _)| n == "content-length")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    let mut body = buffer[head_end..].to_vec();
    while body.len() < length {
        let n = stream.read(&mut byte).await.ok()?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&byte[..n]);
    }
    Some(Recorded {
        method,
        target,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
        closed_by_client: false,
    })
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        401 => "Unauthorized",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        _ => "Status",
    }
}
