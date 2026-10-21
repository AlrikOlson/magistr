use crate::{OwnedProcess, ProcessOutcome, ProcessSpec};

/// Run a bounded owned process from a synchronous capability, including when
/// its caller is already inside a Tokio runtime. Process ownership, capture,
/// deadlines and group cleanup are identical to asynchronous execution.
pub fn run_blocking(spec: ProcessSpec) -> std::io::Result<ProcessOutcome> {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(async move {
            let process = OwnedProcess::spawn(spec)?;
            Ok(process
                .run(&tokio_util::sync::CancellationToken::new(), |_| {})
                .await)
        })
    })
    .join()
    .map_err(|_| std::io::Error::other("The command worker stopped unexpectedly."))?
}
