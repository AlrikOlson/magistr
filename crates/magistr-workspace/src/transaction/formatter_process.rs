//! A tracked formatter process that settles cancellation before releasing its child.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
};
use tokio_util::sync::CancellationToken;

use super::FormatterOptions;

async fn read_bounded(
    mut stream: impl AsyncRead + Unpin,
    limit: usize,
    name: &str,
) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let count = stream
            .read(&mut buffer)
            .await
            .map_err(|error| format!("read formatter {name}: {error}"))?;
        if count == 0 {
            return Ok(output);
        }
        if count > limit.saturating_sub(output.len()) {
            return Err(format!(
                "formatter {name} output limit exceeded ({limit} bytes)"
            ));
        }
        output.extend_from_slice(&buffer[..count]);
    }
}

pub(super) async fn run(
    binary: PathBuf,
    arguments: Vec<String>,
    cwd: PathBuf,
    input: String,
    options: FormatterOptions,
    pinned_toolchain: bool,
    cancel: CancellationToken,
) -> Result<String, String> {
    let mut command = Command::new(&binary);
    if binary == Path::new("rustup") && pinned_toolchain {
        // Cargo exports its own toolchain to child processes. That launch context
        // must not override a formatter workspace's explicit toolchain file.
        command.env_remove("RUSTUP_TOOLCHAIN");
    }
    let mut child = command
        .args(&arguments)
        .current_dir(&cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("start formatter {}: {error}", binary.display()))?;
    let mut stdin = child.stdin.take().ok_or("formatter stdin unavailable")?;
    let stdout = child.stdout.take().ok_or("formatter stdout unavailable")?;
    let stderr = child.stderr.take().ok_or("formatter stderr unavailable")?;
    let operation = async {
        let write = async {
            let result = stdin.write_all(input.as_bytes()).await;
            drop(stdin);
            // The child status and diagnostics explain early rejection more clearly
            // than EPIPE; other I/O errors still reject the candidate.
            match result {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
                Err(error) => Err(format!("write formatter stdin: {error}")),
            }
        };
        tokio::try_join!(
            write,
            read_bounded(stdout, options.max_output_bytes, "stdout"),
            read_bounded(stderr, options.max_output_bytes, "stderr"),
            async {
                child
                    .wait()
                    .await
                    .map_err(|error| format!("wait for formatter: {error}"))
            },
        )
    };
    let result = tokio::select! {
        result = tokio::time::timeout(options.timeout, operation) => result.map_err(|_| format!("formatter timed out after {} ms", options.timeout.as_millis())),
        _ = cancel.cancelled() => Err("formatter cancelled before completion".to_owned()),
    };
    let (_, output, diagnostics, status) = match result {
        Ok(Ok(output)) => output,
        result => {
            let error = match result {
                Ok(Err(error)) => error,
                Err(error) => error,
                Ok(Ok(_)) => unreachable!("success handled above"),
            };
            // The tracked task owns the child even when its caller is gone.
            // Always await reaping, including if signalling reports a failure.
            let killed = child.start_kill();
            let reaped = child.wait().await;
            return Err(match (killed, reaped) {
                (Ok(()), Ok(_)) => error,
                (killed, reaped) => {
                    format!("{error}; formatter cleanup failed: kill={killed:?}, wait={reaped:?}")
                }
            });
        }
    };
    if !status.success() {
        return Err(format!(
            "formatter {} exited {status}: {}",
            binary.display(),
            String::from_utf8_lossy(&diagnostics).trim()
        ));
    }
    String::from_utf8(output).map_err(|error| format!("formatter output is not UTF-8: {error}"))
}
