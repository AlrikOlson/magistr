use super::{WorkerError, WorkerRequest, WorkerResponse};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const VERSION: u32 = 1;
const MAX_FRAME_BYTES: usize = 32 * 1024 * 1024;

/// IPC only. Native callers use typed requests directly.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Frame {
    Ready {
        v: u32,
    },
    Request {
        v: u32,
        id: u64,
        request: WorkerRequest,
    },
    Response {
        v: u32,
        id: u64,
        response: WorkerResponse,
    },
    Error {
        v: u32,
        id: u64,
        error: WorkerError,
    },
    Shutdown {
        v: u32,
    },
}

impl Frame {
    pub(crate) fn version() -> u32 {
        VERSION
    }

    pub(crate) async fn read(reader: &mut (impl AsyncRead + Unpin)) -> Result<Self, WorkerError> {
        // EOF between frames is worker loss; a partial frame is corruption.
        let first = reader
            .read_u8()
            .await
            .map_err(|e| WorkerError::Unavailable(e.to_string()))?;
        let mut prefix = [first, 0, 0, 0];
        reader
            .read_exact(&mut prefix[1..])
            .await
            .map_err(|e| WorkerError::Protocol(e.to_string()))?;
        let length = u32::from_le_bytes(prefix) as usize;
        if length == 0 || length > MAX_FRAME_BYTES {
            return Err(WorkerError::Protocol(
                "frame length outside 1..=32 MiB".into(),
            ));
        }
        let mut bytes = vec![0; length];
        reader
            .read_exact(&mut bytes)
            .await
            .map_err(|e| WorkerError::Protocol(e.to_string()))?;
        let frame: Self =
            serde_json::from_slice(&bytes).map_err(|e| WorkerError::Protocol(e.to_string()))?;
        let v = match &frame {
            Self::Ready { v }
            | Self::Request { v, .. }
            | Self::Response { v, .. }
            | Self::Error { v, .. }
            | Self::Shutdown { v } => *v,
        };
        if v != VERSION {
            return Err(WorkerError::UnsupportedVersion);
        }
        Ok(frame)
    }

    pub(crate) fn encode(&self) -> Result<Vec<u8>, WorkerError> {
        let mut buffer = BoundedBuffer(Vec::new());
        serde_json::to_writer(&mut buffer, self)
            .map_err(|e| WorkerError::Protocol(e.to_string()))?;
        let length = buffer.0.len() as u32;
        let mut bytes = Vec::with_capacity(buffer.0.len() + 4);
        bytes.extend_from_slice(&length.to_le_bytes());
        bytes.extend(buffer.0);
        Ok(bytes)
    }

    pub(crate) async fn write(
        &self,
        writer: &mut (impl AsyncWrite + Unpin),
    ) -> Result<(), WorkerError> {
        let bytes = self.encode()?;
        writer
            .write_all(&bytes)
            .await
            .map_err(|e| WorkerError::Unavailable(e.to_string()))?;
        writer
            .flush()
            .await
            .map_err(|e| WorkerError::Unavailable(e.to_string()))
    }
}

struct BoundedBuffer(Vec<u8>);
impl std::io::Write for BoundedBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_FRAME_BYTES - self.0.len() {
            return Err(std::io::Error::other("worker frame exceeds 32 MiB"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "frame.tests.rs"]
mod tests;
