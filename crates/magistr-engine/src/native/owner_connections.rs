//! Connections whose complete lifetime belongs to one local endpoint.

use std::{
    io,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;

use super::owner_wire::{ClientFrame, CommandRefusal, Greeting, Hello, ReplyFrame};
use super::{
    AttachKind, AttachRefusal, BuildIdentity, McpAcceptor, NativeEngines, OwnerConnectionRecord,
    OwnerGeneration, OwnerIdentity, OwnerReport, ProjectAuthority,
};

/// The admitted service and writer guard. Connection tasks only retain this
/// value while checking admission or executing a typed command. MCP futures
/// receive a child cancellation token and must join their own service work.
pub(super) struct OwnerConnections {
    identity: OwnerIdentity,
    _authority: Option<Arc<ProjectAuthority>>,
    engines: Option<NativeEngines>,
    generation: OwnerGeneration,
    build: BuildIdentity,
    uid: u32,
    mcp: Option<McpAcceptor>,
    /// Every welcomed `mcp` or `commands` session still running, keyed so
    /// its guard removes exactly itself. Probes are answered and closed
    /// without ever entering; the report a probe gets is this list.
    sessions: Mutex<Vec<(u64, OwnerConnectionRecord)>>,
    next_session: AtomicU64,
}

/// Registers one welcomed session for the report and removes it when the
/// session's work — the MCP acceptor's future or the command loop — ends,
/// however it ends.
struct SessionGuard {
    owner: Weak<OwnerConnections>,
    key: u64,
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.upgrade()
            && let Ok(mut sessions) = owner.sessions.lock()
        {
            sessions.retain(|(key, _)| *key != self.key);
        }
    }
}

impl OwnerConnections {
    pub(super) fn new(
        identity: OwnerIdentity,
        authority: Option<Arc<ProjectAuthority>>,
        engines: Option<NativeEngines>,
        generation: OwnerGeneration,
        uid: u32,
        mcp: Option<McpAcceptor>,
    ) -> Self {
        Self {
            identity,
            _authority: authority,
            engines,
            generation,
            build: BuildIdentity::this_build(),
            uid,
            mcp,
            sessions: Mutex::new(Vec::new()),
            next_session: AtomicU64::new(1),
        }
    }

    /// What a probe is told: this owner as it is right now.
    fn report(&self) -> OwnerReport {
        let connections = self
            .sessions
            .lock()
            .map(|sessions| sessions.iter().map(|(_, record)| record.clone()).collect())
            .unwrap_or_default();
        OwnerReport {
            generation: self.generation,
            owner_pid: std::process::id(),
            build: self.build.clone(),
            identity: self.identity.clone(),
            connections,
        }
    }

    /// Enter a welcomed session into the report until its guard drops.
    fn register(self: &Arc<Self>, hello: &Hello) -> SessionGuard {
        let key = self.next_session.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.push((
                key,
                OwnerConnectionRecord {
                    attach: hello.attach,
                    caller: hello.caller.clone(),
                },
            ));
        }
        SessionGuard {
            owner: Arc::downgrade(self),
            key,
        }
    }

    const CONNECTION_LIMIT: usize = 128;

    pub(super) async fn serve(listener: UnixListener, shared: Weak<Self>, stop: CancellationToken) {
        let mut connections = JoinSet::new();
        loop {
            tokio::select! {
                biased;
                _ = stop.cancelled() => break,
                Some(_) = connections.join_next(), if !connections.is_empty() => {},
                accepted = listener.accept() => match accepted {
                    Ok((stream, _)) => {
                        if connections.len() >= Self::CONNECTION_LIMIT {
                            // Admission counts idle handshakes and live sessions
                            // together. Refuse excess peers without spawning work.
                            drop(stream);
                            continue;
                        }
                        connections.spawn(serve_connection(stream, shared.clone(), stop.child_token()));
                    }
                    Err(_) => {
                        tokio::select! {
                            _ = stop.cancelled() => break,
                            _ = tokio::time::sleep(Duration::from_millis(50)) => {},
                        }
                    }
                }
            }
        }
        drop(listener);
        // Cancelling an MCP waiter can detach its rmcp task. Let acceptors
        // observe their token and join cleanup before this endpoint settles.
        while connections.join_next().await.is_some() {}
    }

    fn admit(&self, stream: &UnixStream, hello: &Hello) -> Result<(), AttachRefusal> {
        let peer = stream
            .peer_cred()
            .map_err(|error| AttachRefusal::Caller(error.to_string()))?;
        if peer.uid() != self.uid {
            return Err(AttachRefusal::Caller(format!(
                "uid {} is not the owner's user",
                peer.uid()
            )));
        }
        // The protocol version was decided when the line was parsed
        // (`Hello::parse`); a `Hello` that reaches here speaks this one.
        self.build.admits(&hello.build)?;
        self.identity.admit(&hello.identity)?;
        let offered = match hello.attach {
            AttachKind::Mcp => self.mcp.is_some(),
            AttachKind::Commands => self.engines.is_some(),
            AttachKind::Probe => true,
        };
        if !offered {
            return Err(AttachRefusal::Unsupported(hello.attach));
        }
        Ok(())
    }
}

async fn serve_connection(
    mut stream: UnixStream,
    shared: Weak<OwnerConnections>,
    stop: CancellationToken,
) {
    let line = tokio::select! {
        _ = stop.cancelled() => return,
        result = tokio::time::timeout(Duration::from_secs(5), read_hello_line(&mut stream)) => match result {
            Ok(Ok(line)) => line,
            _ => return,
        }
    };
    let Some(owner) = shared.upgrade() else {
        return;
    };
    // A well-formed line of another protocol version is answered, not
    // dropped: the client learns which version is served. Anything that is
    // not a `Hello` at all gets no greeting.
    let (hello, greeting) = match Hello::parse(&line) {
        Ok(hello) => {
            let greeting = match owner.admit(&stream, &hello) {
                Ok(()) => Greeting::Welcome {
                    generation: owner.generation,
                    owner_pid: std::process::id(),
                },
                Err(refusal) => Greeting::Refused(refusal),
            };
            (Some(hello), greeting)
        }
        Err(Some(refusal)) => (None, Greeting::Refused(refusal)),
        Err(None) => return,
    };
    let welcomed = matches!(greeting, Greeting::Welcome { .. });
    let result = tokio::select! {
        _ = stop.cancelled() => return,
        result = write_line(&mut stream, &greeting) => result,
    };
    let Some(hello) = hello.filter(|_| result.is_ok() && welcomed) else {
        return;
    };
    match hello.attach {
        AttachKind::Probe => {
            // The report is a moment's observation and the whole answer;
            // the stream closes when this task returns.
            let report = owner.report();
            drop(owner);
            tokio::select! {
                _ = stop.cancelled() => {},
                _ = write_line(&mut stream, &report) => {},
            }
        }
        AttachKind::Mcp => {
            let session = owner.register(&hello);
            let acceptor = owner.mcp.clone();
            drop(owner);
            if let Some(acceptor) = acceptor {
                acceptor(stream, stop).await;
            }
            drop(session);
        }
        AttachKind::Commands => {
            let session = owner.register(&hello);
            drop(owner);
            tokio::select! {
                _ = stop.cancelled() => {},
                _ = command_loop(stream, shared) => {},
            }
            drop(session);
        }
    }
}

/// Read exactly the handshake line, without consuming the MCP bytes after it.
async fn read_hello_line(stream: &mut UnixStream) -> io::Result<Vec<u8>> {
    const MAX_HELLO: usize = 64 * 1024;
    let mut line = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        stream.read_exact(&mut byte).await?;
        if byte[0] == b'\n' {
            break;
        }
        line.push(byte[0]);
        if line.len() > MAX_HELLO {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "handshake line too long",
            ));
        }
    }
    Ok(line)
}

async fn write_line<T: serde::Serialize>(
    writer: &mut (impl AsyncWriteExt + Unpin),
    value: &T,
) -> io::Result<()> {
    let mut bytes = serde_json::to_vec(value)?;
    bytes.push(b'\n');
    writer.write_all(&bytes).await?;
    writer.flush().await
}

async fn command_loop(stream: UnixStream, shared: Weak<OwnerConnections>) -> io::Result<()> {
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    while let Some(line) = lines.next_line().await? {
        let frame: ClientFrame = serde_json::from_str(&line)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let ClientFrame::Command {
            id,
            generation,
            command,
        } = frame
        else {
            return Ok(());
        };
        let Some(owner) = shared.upgrade() else {
            return Ok(());
        };
        let outcome = if generation != owner.generation {
            Err(CommandRefusal::StaleGeneration {
                current: owner.generation,
            })
        } else if let Some(engines) = &owner.engines {
            engines.apply(*command).map_err(CommandRefusal::Engine)
        } else {
            Err(CommandRefusal::Engine(
                "this owner serves no typed commands".to_owned(),
            ))
        };
        drop(owner);
        write_line(&mut write, &ReplyFrame { id, outcome }).await?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "owner_connections.tests.rs"]
mod tests;
