use super::{WorkerError, WorkerRequest, frame::Frame};

/// Private executable role. Does not admit a workspace or install application hooks.
pub async fn serve_worker() -> Result<(), WorkerError> {
    let result = serve().await;
    #[cfg(feature = "sandbox")]
    crate::sandbox::server_pool_shutdown();
    result
}

async fn serve() -> Result<(), WorkerError> {
    // Real parser startup self-test before readiness.
    WorkerRequest::Ping.execute()?;
    let mut stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    Frame::Ready {
        v: Frame::version(),
    }
    .write(&mut stdout)
    .await?;
    loop {
        match Frame::read(&mut stdin).await? {
            Frame::Request { id, request, .. } => {
                // Guests run on a blocking thread, never on the runtime thread
                // that drives this loop: the WASI lane blocks on its own inner
                // runtime, which a tokio worker thread refuses. A panic while
                // executing still ends the worker, so the supervisor's crash
                // and quarantine handling is unchanged.
                let frame = match tokio::task::spawn_blocking(move || request.execute()).await {
                    Ok(Ok(response)) => Frame::Response {
                        v: Frame::version(),
                        id,
                        response,
                    },
                    Ok(Err(error)) => Frame::Error {
                        v: Frame::version(),
                        id,
                        error,
                    },
                    Err(join) => {
                        return Err(WorkerError::Unavailable(format!(
                            "worker request panicked: {join}"
                        )));
                    }
                };
                frame.write(&mut stdout).await?;
            }
            Frame::Shutdown { .. } => return Ok(()),
            _ => {
                return Err(WorkerError::Protocol(
                    "worker accepts requests or shutdown only".into(),
                ));
            }
        }
    }
}
