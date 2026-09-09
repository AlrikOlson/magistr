//! The agent's stdio as an SDK transport component.

use agent_client_protocol::Lines;
use futures::{AsyncBufReadExt, AsyncWriteExt, Sink, Stream, StreamExt, io::BufReader};
use tokio::process::{ChildStdin, ChildStdout};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

/// Wrap the child's stdin/stdout as newline-delimited JSON-RPC, tapping every
/// raw line into `tracing` (target `acp_wire`) the way Zed keeps its ACP log.
pub(crate) fn agent_lines(
    stdin: ChildStdin,
    stdout: ChildStdout,
) -> Lines<
    impl Sink<String, Error = std::io::Error> + Send + 'static,
    impl Stream<Item = std::io::Result<String>> + Send + 'static,
> {
    let incoming = BufReader::new(stdout.compat())
        .lines()
        .inspect(|line| match line {
            Ok(line) => tracing::trace!(target: "acp_wire", dir = "in", "{line}"),
            Err(err) => tracing::debug!(target: "acp_wire", "read error: {err}"),
        });

    let outgoing = futures::sink::unfold(
        Box::pin(stdin.compat_write()),
        async move |mut writer, line: String| {
            tracing::trace!(target: "acp_wire", dir = "out", "{line}");
            let mut bytes = line.into_bytes();
            bytes.push(b'\n');
            writer.write_all(&bytes).await?;
            writer.flush().await?;
            Ok::<_, std::io::Error>(writer)
        },
    );

    Lines::new(outgoing, incoming)
}
