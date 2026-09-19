//! Live interface admission and lifetime: the provider, project owners and feeds
//! close together. Surfaces supply only a model, inbox and command receiver.

use std::future::Future;
use std::time::Duration;

use color_eyre::eyre::{self, Context as _};
use magistr_application::sessions::SessionHandle;
use magistr_cockpit::cockpit::{Cmd, Model};
use magistr_cockpit_runtime::channel::{Commands, Inbox};
use magistr_engine::OwnerEndpoint;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use super::{WorkspaceAdmission, workflow_client::EngineClient};
use crate::agent::{AgentConfig, ResolvedSessionSpec, cockpit_session::CockpitSession};
use crate::orchestration::{interactive_session::interactive_session, loop_driver::Connector};

const CLOSE_GRACE: Duration = Duration::from_secs(5);

/// A live session host, independent of the interface that displays its model.
///
/// Dropping this handle requests the same bounded close as `shutdown`. The
/// cleanup task retains the admitted project until the session has settled.
/// Keep the Tokio runtime alive and await `shutdown` before exiting the process.
pub struct InteractiveHost {
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<(), String>>,
}

impl InteractiveHost {
    /// Admit or attach to the project, start the configured provider, and seed
    /// the interface. No terminal or window is opened here.
    ///
    /// # Errors
    /// Project admission, provider configuration or provider startup failed.
    pub async fn start(
        config: AgentConfig,
        model: Model,
        inbox: Inbox,
        commands: Commands,
    ) -> eyre::Result<(Self, Model)> {
        let engines = EngineClient::admit_or_attach_config(
            magistr_core::evidence::writer_config_from_env(&config.cwd),
        )
        .context("Magistr could not open the project")?;
        let resources = Resources::open(engines, config.inputs()).await?;
        let spec = config.clone().resolve();
        Self::connect(
            config,
            resources,
            spec,
            model,
            inbox,
            commands,
            |spec| async {
                CockpitSession::start(spec)
                    .await
                    .map(|session| Box::new(session) as Box<dyn SessionHandle>)
                    .map_err(eyre::Report::from)
            },
        )
        .await
    }

    async fn connect<F, Fut>(
        config: AgentConfig,
        resources: Resources,
        spec: crate::error::Result<ResolvedSessionSpec>,
        model: Model,
        inbox: Inbox,
        commands: Commands,
        connect: F,
    ) -> eyre::Result<(Self, Model)>
    where
        F: FnOnce(ResolvedSessionSpec) -> Fut,
        Fut: Future<Output = eyre::Result<Box<dyn SessionHandle>>>,
    {
        let source = resources.workspace.source_route();
        let spec = match spec {
            Ok(spec) => spec.with_source_route(source.clone()),
            Err(error) => {
                drop(source);
                let cleanup = resources.close().await;
                return Err(startup_error(error.into(), cleanup));
            }
        };
        let (title, debug) = crate::commands::chrome::chrome(spec.cwd(), Some("acp"), spec.model());
        let project = spec.cwd().file_name().map_or_else(
            || spec.cwd().display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        let model = crate::commands::run::seed_supervision(
            model
                .with_project(project, spec.cwd().display().to_string())
                .with_title(title)
                .with_debug(debug),
            spec.cwd(),
            spec.permission_profile(),
            spec.workflow_permissions(),
            config.inputs(),
        );
        let handle = match connect(spec).await {
            Ok(handle) => handle,
            Err(error) => {
                // Release the provider's source reference before the service.
                drop(source);
                let cleanup = resources.close().await;
                return Err(startup_error(error, cleanup));
            }
        };
        let opening = !config.prompt.trim().is_empty();
        let seed = resources.engines.reader().snapshot();
        let model = model.with_snapshot(&seed).with_workspace_files(
            crate::commands::workspace_files::workspace_files(&config.cwd),
        );
        let feed = OwnedTask(resources.engines.subscribe(inbox.clone()));
        let reseed = OwnedTask(resources.engines.reseed(inbox.clone(), seed.cursor));
        let session = interactive_session(
            config,
            Connector::live(source),
            inbox.clone(),
            Some(resources.engines.clone()),
        )
        .connected(handle, opening);
        let (forward, receiver) = Commands::channel();
        let relay = OwnedTask(session.spawn(receiver));
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            let result = serve(commands, forward, stopped, relay, inbox).await;
            feed.abort_and_join().await;
            reseed.abort_and_join().await;
            let cleanup = resources.close().await;
            let result = combine(result, cleanup);
            if let Err(error) = &result {
                tracing::warn!(%error, "interactive session close failed");
            }
            result
        });
        Ok((
            Self {
                stop: Some(stop),
                task,
            },
            model,
        ))
    }

    /// Stop accepting interface commands and await session and project cleanup.
    ///
    /// # Errors
    /// The session exceeded its close deadline or project cleanup failed.
    pub async fn shutdown(mut self) -> Result<(), String> {
        self.request_close();
        (&mut self.task)
            .await
            .map_err(|error| format!("The session could not close: {error}"))?
    }

    fn request_close(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

impl Drop for InteractiveHost {
    fn drop(&mut self) {
        self.request_close();
    }
}

struct Resources {
    endpoint: Option<OwnerEndpoint>,
    workspace: WorkspaceAdmission,
    engines: EngineClient,
}

impl Resources {
    async fn open(
        engines: EngineClient,
        inputs: &crate::config::inputs::ProjectInputs,
    ) -> eyre::Result<Self> {
        engines
            .port()
            .ok_or_else(|| eyre::eyre!("Magistr could not connect to the project"))?;
        let endpoint = engines.serve_owner().await?;
        let workspace = WorkspaceAdmission::admit(inputs).await;
        Ok(Self {
            endpoint,
            workspace,
            engines,
        })
    }

    async fn close(self) -> Result<(), String> {
        let endpoint = match self.endpoint {
            Some(endpoint) => endpoint.shutdown().await.map_err(|error| error.to_string()),
            None => Ok(()),
        };
        let workspace = self
            .workspace
            .shutdown()
            .await
            .map_err(|error| error.to_string());
        // The workflow writer is released only after its endpoint and workspace.
        drop(self.engines);
        combine(endpoint, workspace)
    }
}

/// Every spawned worker owned by the cleanup task cancels if that task itself
/// is aborted (including runtime shutdown). Normal close also awaits it.
struct OwnedTask<T>(JoinHandle<T>);

impl<T> Drop for OwnedTask<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl<T> OwnedTask<T> {
    async fn abort_and_join(mut self) {
        self.0.abort();
        let _ = (&mut self.0).await;
    }
}

async fn serve(
    mut commands: Commands,
    forward: tokio::sync::mpsc::UnboundedSender<Cmd>,
    mut stopped: oneshot::Receiver<()>,
    mut relay: OwnedTask<Result<(), String>>,
    inbox: Inbox,
) -> Result<(), String> {
    loop {
        tokio::select! {
            biased;
            _ = &mut stopped => break,
            result = &mut relay.0 => return joined(result),
            command = commands.next() => {
                match command {
                    None | Some(Cmd::Quit) => break,
                    Some(command) => {
                        if let Err(error) = forward.send(command) {
                            inbox.send(magistr_cockpit::cockpit::Msg::CommandFailed {
                                cmd: error.0,
                                error: "The session connection closed. Reopen the session and try again.".into(),
                            });
                            break;
                        }
                    }
                }
            }
        }
    }
    // Quit overtakes queued commands on close: discard the external receiver.
    drop(commands);
    let _ = forward.send(Cmd::Quit);
    drop(forward);
    match tokio::time::timeout(CLOSE_GRACE, &mut relay.0).await {
        Ok(result) => joined(result),
        Err(_) => {
            relay.abort_and_join().await;
            Err("The agent took too long to close and was stopped.".into())
        }
    }
}

fn joined(result: Result<Result<(), String>, tokio::task::JoinError>) -> Result<(), String> {
    result.map_err(|error| format!("The agent connection ended unexpectedly: {error}"))?
}

fn combine(first: Result<(), String>, second: Result<(), String>) -> Result<(), String> {
    match (first, second) {
        (Ok(()), result) | (result, Ok(())) => result,
        (Err(first), Err(second)) => Err(format!("{first}; {second}")),
    }
}

fn startup_error(error: eyre::Report, cleanup: Result<(), String>) -> eyre::Report {
    match cleanup {
        Ok(()) => error.wrap_err("Magistr could not start the session"),
        Err(cleanup) => error.wrap_err(format!(
            "Magistr could not start the session. The project connection could not close: {cleanup}"
        )),
    }
}

#[cfg(test)]
#[path = "interactive_host.tests.rs"]
mod tests;
