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
                let frame = match request.execute() {
                    Ok(response) => Frame::Response {
                        v: Frame::version(),
                        id,
                        response,
                    },
                    Err(error) => Frame::Error {
                        v: Frame::version(),
                        id,
                        error,
                    },
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
