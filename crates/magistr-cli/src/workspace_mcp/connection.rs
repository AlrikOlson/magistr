//! Admission is workspace-wide, independent of workflow writer roles.
use std::sync::Arc;
#[cfg(unix)]
use std::time::Duration;

#[cfg(unix)]
use magistr_engine::native::{AttachError, CallerIdentity, OwnerClient};
use magistr_workspace::service::{WorkspaceOpenError, WorkspaceOpenOptions, WorkspaceService};

/// Startup either owns the native service or holds only a welcomed transport.
pub(super) enum WorkspaceConnection {
    Owner(Arc<WorkspaceService>),
    #[cfg(unix)]
    Attached {
        generation: magistr_engine::native::OwnerGeneration,
        stream: tokio::net::UnixStream,
    },
}

impl WorkspaceConnection {
    /// Retry only native contention during the bounded lock-to-listener window.
    /// A migration refusal never grants permission to contact an endpoint.
    #[cfg(unix)]
    pub(super) async fn open(options: WorkspaceOpenOptions) -> Result<Self, WorkspaceOpenError> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            match WorkspaceService::open(options.clone()).await {
                Ok(service) => return Ok(Self::Owner(Arc::new(service))),
                Err(WorkspaceOpenError::Locked(refusal)) => {
                    if !options.session_flags.is_empty() {
                        return Err(WorkspaceOpenError::Locked(format!(
                            "{refusal}; attachment cannot change the running owner's --config; \
                             reconnect without overrides or stop that owner first"
                        )));
                    }
                    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                    if remaining.is_zero() {
                        return Err(WorkspaceOpenError::Locked(format!(
                            "{refusal}; workspace attachment timed out after 5 seconds"
                        )));
                    }
                    let attached = OwnerClient::attach_workspace(
                        &options.root,
                        CallerIdentity::this_process("workspace serve"),
                        remaining.min(Duration::from_millis(250)),
                    )
                    .await;
                    match attached {
                        Ok((generation, stream)) => {
                            tracing::info!(
                                generation = generation.get(),
                                "attached to native workspace owner"
                            );
                            return Ok(Self::Attached { generation, stream });
                        }
                        Err(AttachError::Unavailable(_) | AttachError::Deadline(_)) => {
                            tokio::time::sleep(
                                deadline
                                    .saturating_duration_since(tokio::time::Instant::now())
                                    .min(Duration::from_millis(50)),
                            )
                            .await;
                        }
                        Err(error) => {
                            return Err(WorkspaceOpenError::Locked(format!(
                                "{refusal}; workspace attachment refused: {error}"
                            )));
                        }
                    }
                }
                Err(error) => return Err(error),
            }
        }
    }
    #[cfg(not(unix))]
    pub(super) async fn open(options: WorkspaceOpenOptions) -> Result<Self, WorkspaceOpenError> {
        WorkspaceService::open(options)
            .await
            .map(|service| Self::Owner(Arc::new(service)))
    }
}
