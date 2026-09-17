//! One absolute deadline for a workspace connection and its greeting.

use std::{io, path::Path, time::Duration};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
};

use super::owner_wire::{Greeting, Hello, OWNER_PROTOCOL_VERSION};
use super::{
    AttachError, AttachKind, BuildIdentity, CallerIdentity, OwnerGeneration, OwnerIdentity,
    workspace_socket_path,
};

/// Connect without an uncancellable blocking socket operation. The deadline
/// covers connect, hello write, and the entire greeting, including slow peers.
pub(super) async fn attach(
    root: &Path,
    caller: CallerIdentity,
    deadline: Duration,
) -> Result<(OwnerGeneration, UnixStream), AttachError> {
    let root = root.canonicalize().map_err(AttachError::Unavailable)?;
    let path = workspace_socket_path(&root).map_err(AttachError::Unavailable)?;
    let hello = Hello {
        protocol: OWNER_PROTOCOL_VERSION,
        build: BuildIdentity::this_build(),
        identity: OwnerIdentity::Workspace { root },
        attach: AttachKind::Mcp,
        caller,
    };
    tokio::time::timeout(deadline, exchange(&path, &hello))
        .await
        .map_err(|_| AttachError::Deadline(deadline))?
}

async fn exchange(
    path: &Path,
    hello: &Hello,
) -> Result<(OwnerGeneration, UnixStream), AttachError> {
    let mut stream = UnixStream::connect(path)
        .await
        .map_err(AttachError::Unavailable)?;
    let mut bytes =
        serde_json::to_vec(hello).map_err(|error| AttachError::Wire(error.to_string()))?;
    bytes.push(b'\n');
    stream.write_all(&bytes).await.map_err(link_error)?;
    stream.flush().await.map_err(link_error)?;
    let mut line = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        stream.read_exact(&mut byte).await.map_err(link_error)?;
        if byte[0] == b'\n' {
            break;
        }
        line.push(byte[0]);
        if line.len() > 64 * 1024 {
            return Err(AttachError::Wire(
                "owner greeting exceeds 64 KiB".to_owned(),
            ));
        }
    }
    let greeting: Greeting =
        serde_json::from_slice(&line).map_err(|error| AttachError::Wire(error.to_string()))?;
    match greeting {
        Greeting::Welcome { generation, .. } => Ok((generation, stream)),
        Greeting::Refused(refusal) => Err(AttachError::Refused(refusal)),
    }
}

fn link_error(error: io::Error) -> AttachError {
    if matches!(
        error.kind(),
        io::ErrorKind::UnexpectedEof
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::BrokenPipe
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::NotConnected
    ) {
        AttachError::OwnerGone
    } else {
        AttachError::Unavailable(error)
    }
}

#[cfg(test)]
#[path = "owner_workspace_link.tests.rs"]
mod tests;
