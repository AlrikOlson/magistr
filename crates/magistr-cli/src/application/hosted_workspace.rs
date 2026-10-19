use std::sync::Arc;

use magistr_application::effects::SourceWriter;
use magistr_workspace::{
    contracts::WorkspaceError,
    service::{WorkspaceOpenError, WorkspaceService},
};

use super::WorkspaceSourceWriter;
use crate::config::inputs::ProjectInputs;

/// The workspace this host admitted for its run: the one native owner of
/// the checkout for as long as the run lasts. The host opens the service
/// on the bridge's own open options and serves the shared owner endpoint,
/// so the agent's `magistr` bridge and the cockpit's port attach to
/// this process instead of admitting; its own source writes go through
/// the service in-process. When another owner already holds the lock the
/// host admits nothing — it never attaches to write through a socket and
/// never opens a second owner — and its sessions' source writes are
/// refused (see [`super::WorkspaceAdmission`]).
pub struct HostedWorkspace {
    service: Arc<WorkspaceService>,
    #[cfg(any(unix, windows))]
    endpoint: Option<magistr_engine::native::OwnerEndpoint>,
}

impl HostedWorkspace {
    /// Admit the captured project's root in this process and serve the owner
    /// endpoint. `Ok(None)` when a live owner holds the workspace.
    ///
    /// # Errors
    /// The root cannot be admitted for another reason: a missing root, a
    /// marker that needs migration, durable data from a newer binary, a
    /// configuration refusal, or an endpoint that could not be served.
    pub async fn admit(inputs: &ProjectInputs) -> Result<Option<Self>, WorkspaceOpenError> {
        let options = crate::workspace_mcp::open_options(inputs, serde_json::Map::new())?;
        Self::admit_with(options).await
    }

    /// As [`Self::admit`] on explicit open options — the seam a test uses
    /// to name the real worker binary instead of this process.
    ///
    /// # Errors
    /// As [`Self::admit`].
    pub async fn admit_with(
        mut options: magistr_workspace::service::WorkspaceOpenOptions,
    ) -> Result<Option<Self>, WorkspaceOpenError> {
        magistr_engine::native::ProjectLocation::require_open(&options.root)?;
        let root = options.root.clone();
        options.lifecycle_lock = Some(
            magistr_engine::native::ProjectLocation::working_copy_root(&root)?
                .join(".magistr/working-copy.lock"),
        );
        let service = match WorkspaceService::open(options).await {
            Ok(service) => {
                if let Err(error) = magistr_engine::native::ProjectLocation::require_open(&root) {
                    let _ = service.shutdown().await;
                    return Err(error.into());
                }
                Arc::new(service)
            }
            Err(WorkspaceOpenError::Locked(_)) => return Ok(None),
            Err(error) => return Err(error),
        };
        #[cfg(any(unix, windows))]
        let endpoint = match crate::workspace_mcp::serve_owner_endpoint(Arc::clone(&service)).await
        {
            Ok(endpoint) => Some(endpoint),
            Err(error) => {
                if let Ok(service) = Arc::try_unwrap(service) {
                    let _ = service.shutdown().await;
                }
                return Err(WorkspaceOpenError::Io(error));
            }
        };
        Ok(Some(Self {
            service,
            #[cfg(any(unix, windows))]
            endpoint,
        }))
    }

    /// The admitted service.
    #[must_use]
    pub fn service(&self) -> &Arc<WorkspaceService> {
        &self.service
    }

    /// The ownership incarnation this host holds.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.service.generation().get()
    }

    /// The route for this host's own source writes: a writer over the
    /// admitted service under a fresh journal session.
    ///
    /// # Errors
    /// The service refused to mint a session.
    pub fn source_writer(&self) -> Result<Arc<dyn SourceWriter>, WorkspaceError> {
        WorkspaceSourceWriter::new(Arc::clone(&self.service))
            .map(|writer| Arc::new(writer) as Arc<dyn SourceWriter>)
    }

    /// Stop taking clients, then shut the workspace down in the order the
    /// service requires. A reference still held elsewhere (a session that
    /// has not released its writer yet) is reported instead of leaking a
    /// half-closed owner.
    ///
    /// # Errors
    /// The endpoint or the service refused to shut down, or references to
    /// the service remain.
    pub async fn shutdown(self) -> Result<(), WorkspaceError> {
        #[cfg(any(unix, windows))]
        if let Some(endpoint) = self.endpoint {
            endpoint.shutdown().await.map_err(|error| {
                WorkspaceError::new(
                    magistr_workspace::contracts::ErrorKind::EngineUnavailable,
                    format!("workspace owner endpoint: {error}"),
                )
            })?;
        }
        let service = Arc::try_unwrap(self.service).map_err(|_| {
            WorkspaceError::new(
                magistr_workspace::contracts::ErrorKind::EngineUnavailable,
                "workspace service references remain after the run; explicit shutdown refused",
            )
        })?;
        service.shutdown().await
    }
}
