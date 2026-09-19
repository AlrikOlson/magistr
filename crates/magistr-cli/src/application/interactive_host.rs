//! Live interface admission and lifetime: the provider, project owners and feeds
//! close together. Surfaces supply only a model, inbox and command receiver.

use std::future::Future;
use std::time::Duration;

use color_eyre::eyre::{self, Context as _};
use magistr_application::sessions::SessionHandle;
use magistr_cockpit::cockpit::{Cmd, Model, ProjectSeed};
use magistr_cockpit_runtime::channel::{Commands, Inbox};
use magistr_engine::OwnerEndpoint;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use super::{
    WorkspaceAdmission,
    workflow_client::{EngineClient, EngineError},
};
use crate::agent::{AgentConfig, ResolvedSessionSpec, cockpit_session::CockpitSession};
use crate::orchestration::{
    hosted_work::HostedWork, interactive_session::interactive_session, loop_driver::Connector,
};

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
    /// [`Self::open`], with the interface's `model` seeded on the caller's thread.
    ///
    /// # Errors
    /// As [`Self::open`].
    pub async fn start(
        config: AgentConfig,
        model: Model,
        inbox: Inbox,
        commands: Commands,
    ) -> eyre::Result<(Self, Model)> {
        let (host, seed) = Self::open(config, inbox, commands).await?;
        Ok((host, seed.seed(model)))
    }

    /// Admit or attach to the project and start the configured provider. The
    /// interface's seed comes back as data: a model cannot cross threads, so
    /// whoever owns the surface builds it. No terminal or window is opened here.
    ///
    /// # Errors
    /// Project admission, provider configuration or provider startup failed.
    pub async fn open(
        config: AgentConfig,
        inbox: Inbox,
        commands: Commands,
    ) -> eyre::Result<(Self, ProjectSeed)> {
        let engines = EngineClient::admit_or_attach_config(
            magistr_core::evidence::writer_config_from_env(&config.cwd),
        )
        .context("Magistr could not open the project")?;
        let resources = Resources::open(engines, config.inputs()).await?;
        Self::live(config, resources, inbox, commands).await
    }

    /// As [`Self::open`] for a project a window switches to: this process
    /// becomes the project's host and owns its workspace, or the open is
    /// refused in words and nothing of the project stays held. A window never
    /// swaps a project it hosts for one it could only attach to.
    ///
    /// # Errors
    /// The host lease or the workspace is held elsewhere, or as [`Self::open`].
    pub async fn open_owned(
        config: AgentConfig,
        inbox: Inbox,
        commands: Commands,
    ) -> eyre::Result<(Self, ProjectSeed)> {
        let engines =
            EngineClient::with_config(magistr_core::evidence::writer_config_from_env(&config.cwd))
                .map_err(|error| match &error {
                    EngineError::Admission { source, .. }
                        if source.kind() == std::io::ErrorKind::WouldBlock =>
                    {
                        eyre::eyre!("Another Magistr session has this project open.")
                    }
                    _ => eyre::Report::new(error).wrap_err("Magistr could not open the project"),
                })?;
        let resources = Resources::open(engines, config.inputs()).await?;
        if let WorkspaceAdmission::Refused(refusal) = &resources.workspace {
            use magistr_application::effects::SourceRefusal;
            let held = match refusal {
                SourceRefusal::NotAdmitted(reason) => {
                    eyre::eyre!("Its source workspace could not be opened: {reason}")
                }
                SourceRefusal::OwnerElsewhere | SourceRefusal::NoAuthority => {
                    eyre::eyre!("Another Magistr process holds this project's source workspace.")
                }
            };
            let cleanup = resources.close().await;
            return Err(startup_error(held, cleanup));
        }
        Self::live(config, resources, inbox, commands).await
    }

    /// The production provider behind admitted `resources`.
    async fn live(
        config: AgentConfig,
        resources: Resources,
        inbox: Inbox,
        commands: Commands,
    ) -> eyre::Result<(Self, ProjectSeed)> {
        let spec = config.clone().resolve();
        Self::connect(
            config,
            resources,
            spec,
            inbox,
            commands,
            |spec| async {
                CockpitSession::start(spec)
                    .await
                    .map(|session| Box::new(session) as Box<dyn SessionHandle>)
                    .map_err(eyre::Report::from)
            },
            Connector::live,
        )
        .await
    }

    /// `connect` opens the first conversation; `sessions` builds the connector
    /// every later conversation and every started work run connects through.
    #[allow(clippy::too_many_arguments)] // the composition seam names each collaborator once
    async fn connect<F, Fut>(
        config: AgentConfig,
        resources: Resources,
        spec: crate::error::Result<ResolvedSessionSpec>,
        inbox: Inbox,
        commands: Commands,
        connect: F,
        sessions: impl FnOnce(magistr_application::effects::SourceRoute) -> Connector,
    ) -> eyre::Result<(Self, ProjectSeed)>
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
        let project = crate::commands::galaxy_data::project_name(spec.cwd());
        let dir = spec.cwd().display().to_string();
        let supervision = crate::commands::run::supervision_seed(
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
        let workspace_files = crate::commands::workspace_files::workspace_files(&config.cwd);
        // The feed is tapped for the work this host may be asked to start:
        // the loop driver counts completed chunks from it while it runs.
        let (feed_tap, tapped) = tokio::sync::mpsc::unbounded_channel();
        let feed = OwnedTask(resources.engines.subscribe(inbox.tap(feed_tap)));
        let reseed = OwnedTask(resources.engines.reseed(inbox.clone(), seed.cursor));
        let connector = sessions(source);
        let session = interactive_session(
            config.clone(),
            connector.clone(),
            inbox.clone(),
            Some(resources.engines.clone()),
        )
        .connected(handle, opening);
        let work = HostedWork::new(
            config,
            connector,
            inbox.clone(),
            resources.engines.clone(),
            tapped,
        );
        let (forward, receiver) = Commands::channel();
        let relay = OwnedTask(tokio::spawn(work.run(session, receiver)));
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
            ProjectSeed {
                name: project,
                dir,
                title,
                debug,
                supervision,
                snapshot: seed,
                workspace_files,
            },
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

pub(super) fn combine(first: Result<(), String>, second: Result<(), String>) -> Result<(), String> {
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
