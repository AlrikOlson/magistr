//! Connections whose complete lifetime belongs to one local endpoint.

use std::{
    io,
    sync::{Arc, Weak},
    time::Duration,
};

use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;

use super::owner_wire::{
    ClientFrame, CommandRefusal, Greeting, Hello, OWNER_BUILD, OWNER_PROTOCOL_VERSION, ReplyFrame,
};
use super::{
    AttachKind, AttachRefusal, McpAcceptor, NativeEngines, OwnerGeneration, OwnerIdentity,
    ProjectAuthority,
};

/// The admitted service and writer guard. Connection tasks only retain this
/// value while checking admission or executing a typed command. MCP futures
/// receive a child cancellation token and must join their own service work.
pub(super) struct OwnerConnections {
    identity: OwnerIdentity,
    _authority: Option<Arc<ProjectAuthority>>,
    engines: Option<NativeEngines>,
    generation: OwnerGeneration,
    uid: u32,
    mcp: Option<McpAcceptor>,
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
            uid,
            mcp,
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
        if hello.protocol != OWNER_PROTOCOL_VERSION {
            return Err(AttachRefusal::Protocol {
                served: OWNER_PROTOCOL_VERSION,
                offered: hello.protocol,
            });
        }
        if hello.build != OWNER_BUILD {
            return Err(AttachRefusal::Build {
                served: OWNER_BUILD.to_owned(),
                offered: hello.build.clone(),
            });
        }
        self.identity.admit(&hello.identity)?;
        let offered = match hello.attach {
            AttachKind::Mcp => self.mcp.is_some(),
            AttachKind::Commands => self.engines.is_some(),
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
    let hello = tokio::select! {
        _ = stop.cancelled() => return,
        result = tokio::time::timeout(Duration::from_secs(5), read_hello(&mut stream)) => match result {
            Ok(Ok(hello)) => hello,
            _ => return,
        }
    };
    let Some(owner) = shared.upgrade() else {
        return;
    };
    let greeting = match owner.admit(&stream, &hello) {
        Ok(()) => Greeting::Welcome {
            generation: owner.generation,
            owner_pid: std::process::id(),
        },
        Err(refusal) => Greeting::Refused(refusal),
    };
    let welcomed = matches!(greeting, Greeting::Welcome { .. });
    let result = tokio::select! {
        _ = stop.cancelled() => return,
        result = write_line(&mut stream, &greeting) => result,
    };
    if result.is_err() || !welcomed {
        return;
    }
    match hello.attach {
        AttachKind::Mcp => {
            let acceptor = owner.mcp.clone();
            drop(owner);
            if let Some(acceptor) = acceptor {
                acceptor(stream, stop).await;
            }
        }
        AttachKind::Commands => {
            drop(owner);
            tokio::select! {
                _ = stop.cancelled() => {},
                _ = command_loop(stream, shared) => {},
            }
        }
    }
}

/// Read exactly the handshake, without consuming the MCP bytes after it.
async fn read_hello(stream: &mut UnixStream) -> io::Result<Hello> {
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
    serde_json::from_slice(&line).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
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
