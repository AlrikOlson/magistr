//! Content-length framed encoding and decoding.
//!
//! Uses the LSP / MCP framing protocol:
//!
//! ```text
//! Content-Length: <n>\r\n
//! \r\n
//! <n bytes of payload>
//! ```
//!
//! The functions operate on generic `AsyncRead` / `AsyncWrite` so they
//! can be reused by any transport implementation.

use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::error::TransportError;

/// The header prefix for content-length framing.
const CONTENT_LENGTH_PREFIX: &str = "Content-Length: ";

/// Write a content-length framed message to the writer.
///
/// Writes the `Content-Length` header followed by `\r\n\r\n` and the payload.
/// Flushes the writer after writing.
pub async fn write_frame(
    writer: &mut (impl AsyncWrite + Unpin),
    payload: &[u8],
) -> Result<(), TransportError> {
    let header = format!("{CONTENT_LENGTH_PREFIX}{}\r\n\r\n", payload.len());
    writer.write_all(header.as_bytes()).await?;
    writer.write_all(payload).await?;
    writer.flush().await?;
    Ok(())
}

/// Read a content-length framed message from a buffered reader.
///
/// The caller must provide a `BufReader` (or any `AsyncBufRead`) so that
/// buffered data is preserved across multiple `read_frame` calls.
///
/// Parses the `Content-Length` header, then reads exactly that many bytes
/// of payload. Returns [`TransportError::ConnectionClosed`] on EOF and
/// [`TransportError::InvalidFrame`] if the header is malformed.
pub async fn read_frame(
    reader: &mut (impl AsyncBufRead + Unpin),
) -> Result<Vec<u8>, TransportError> {
    let content_length = read_content_length_header(reader).await?;

    let mut payload = vec![0u8; content_length];
    reader.read_exact(&mut payload).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            TransportError::ConnectionClosed
        } else {
            TransportError::Io(e)
        }
    })?;

    Ok(payload)
}

/// Parse headers until we find `Content-Length` and consume the blank line.
async fn read_content_length_header(
    reader: &mut (impl AsyncBufReadExt + Unpin),
) -> Result<usize, TransportError> {
    let mut content_length: Option<usize> = None;

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line).await?;

        if bytes_read == 0 {
            return Err(TransportError::ConnectionClosed);
        }

        let trimmed = line.trim_end_matches(['\r', '\n']);

        // Empty line signals end of headers.
        if trimmed.is_empty() {
            return content_length.ok_or_else(|| {
                TransportError::InvalidFrame("missing Content-Length header".into())
            });
        }

        if let Some(value) = trimmed.strip_prefix(CONTENT_LENGTH_PREFIX) {
            content_length = Some(value.parse::<usize>().map_err(|_| {
                TransportError::InvalidFrame(format!("invalid Content-Length value: {value}"))
            })?);
        }
        // Ignore unknown headers for forward compatibility.
    }
}

#[cfg(test)]
#[path = "framing.tests.rs"]
mod tests;
