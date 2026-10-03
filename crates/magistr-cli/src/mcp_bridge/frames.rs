//! Newline MCP framing and half-close, independent of which service owns it.
use std::pin::Pin;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, Lines};

pub(crate) struct Frames {
    reader: Lines<BufReader<Pin<Box<dyn AsyncRead + Send>>>>,
    writer: Option<Pin<Box<dyn AsyncWrite + Send>>>,
}

impl Frames {
    pub(crate) fn new(stream: impl AsyncRead + AsyncWrite + Send + 'static) -> Self {
        let (read, write) = tokio::io::split(stream);
        Self {
            reader: BufReader::new(Box::pin(read) as Pin<Box<dyn AsyncRead + Send>>).lines(),
            writer: Some(Box::pin(write)),
        }
    }
    pub(crate) async fn next_line(&mut self) -> Option<String> {
        self.reader.next_line().await.ok().flatten()
    }
    pub(crate) async fn send(&mut self, line: &str) -> std::io::Result<()> {
        let writer = self.writer.as_mut().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "the client's side of the link is closed",
            )
        })?;
        writer.write_all(line.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await
    }
    pub(crate) async fn half_close(&mut self) {
        if let Some(mut writer) = self.writer.take() {
            let _ = writer.shutdown().await;
        }
    }
}

#[cfg(test)]
#[path = "frames.tests.rs"]
mod tests;
