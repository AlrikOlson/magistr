//! Live interface admission and lifetime: the provider, project owners and feeds
//! close together. Registry sessions share admitted resources; surfaces supply
//! only a model, observation inbox and command receiver.

use std::future::Future;
use std::time::Duration;

use color_eyre::eyre::{self, Context as _};
use magistr_application::sessions::SessionHandle;
use magistr_cockpit::cockpit::{Cmd, ProjectSeed};
use magistr_cockpit_runtime::channel::{Commands, Inbox};
use magistr_engine::OwnerEndpoint;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use super::{WorkspaceAdmission, workflow_client::EngineClient};
use crate::agent::{AgentConfig, ResolvedSessionSpec};
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
        Self::live(config, resources, inbox, commands, None).await
    }

    pub(super) async fn open_admitted(
        config: AgentConfig,
        resources: Resources,
        inbox: Inbox,
        commands: Commands,
        observation: std::sync::Arc<super::session_observation::SessionObservation>,
    ) -> eyre::Result<(Self, ProjectSeed)> {
        Self::live(config, resources, inbox, commands, Some(observation)).await
    }

    /// The production provider behind admitted `resources`.
    async fn live(
        config: AgentConfig,
        resources: Resources,
        inbox: Inbox,
        commands: Commands,
        observation: Option<std::sync::Arc<super::session_observation::SessionObservation>>,
    ) -> eyre::Result<(Self, ProjectSeed)> {
        let spec = config.clone().resolve();
        let observing = observation.clone();
        Self::connect(
            config,
            resources,
            spec,
            inbox,
            commands,
            |spec, connector| async move {
                let transport = spec.connect_transport().await?;
                let options = magistr_application::sessions::SessionOptions {
                    opening_prompt: (!spec.prompt().trim().is_empty())
                        .then(|| spec.prompt().into()),
                    model: None,
                    max_turns: spec.max_turns(),
                };
                connector
                    .start_session(transport, spec, options)
                    .await
                    .map(|session| Box::new(session) as Box<dyn SessionHandle>)
                    .map_err(eyre::Report::from)
            },
            move |source| Connector::live(source).observing(observing),
            observation,
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
        observation: Option<std::sync::Arc<super::session_observation::SessionObservation>>,
    ) -> eyre::Result<(Self, ProjectSeed)>
    where
        F: FnOnce(ResolvedSessionSpec, Connector) -> Fut,
        Fut: Future<Output = eyre::Result<Box<dyn SessionHandle>>>,
    {
        let source = resources.admitted.workspace.source_route();
        let spec = match spec {
            Ok(spec) => spec.with_source_route(source.clone()),
            Err(error) => {
                drop(source);
                let cleanup = resources.close().await;
                return Err(startup_error(error.into(), cleanup));
            }
        };
        let mut selection = spec.selection();
        let project = crate::commands::galaxy_data::project_name(spec.cwd());
        let dir = spec.cwd().display().to_string();
        let supervision = crate::commands::run::supervision_seed(
            spec.cwd(),
            spec.permission_profile(),
            spec.workflow_permissions(),
            config.inputs(),
        );
        let connector = sessions(source.clone());
        let handle = match connect(spec, connector.clone()).await {
            Ok(handle) => handle,
            Err(error) => {
                // Release the provider's source reference before the service.
                drop(connector);
                drop(source);
                let cleanup = resources.close().await;
                return Err(startup_error(error, cleanup));
            }
        };
        // The history owner binds the accepted provider configuration before
        // returning the handle. A requested default may not have been offered.
        selection.model = config
            .history
            .current_model()
            .filter(|model| !model.eq_ignore_ascii_case("default"));
        selection
            .sources
            .insert("model".into(), "current conversation".into());
        let inputs = config
            .inputs()
            .as_ref()
            .clone()
            .with_session_selection(selection);
        let config = config.with_inputs(&inputs);
        let opening = !config.prompt.trim().is_empty();
        let seed = resources.engines.snapshot_reads().read(None, true).await;
        let workspace_files = crate::commands::workspace_files::workspace_files(&config.cwd);
        // The feed is tapped for the work this host may be asked to start:
        // the loop driver counts completed chunks from it while it runs.
        let (feed_tap, tapped) = tokio::sync::mpsc::unbounded_channel();
        let feed = OwnedTask(resources.engines.subscribe(inbox.tap(feed_tap)));
        let reseed = OwnedTask(resources.engines.reseed(inbox.clone()));
        let history_feed = OwnedTask(super::history_feed::follow(
            config.cwd.clone(),
            config.inputs().as_ref().clone(),
            inbox.clone(),
        ));
        drop(source);
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
        )
        .observing(observation.clone());
        let (forward, receiver) = Commands::bounded(32);
        let relay = OwnedTask(tokio::spawn(work.run(session, receiver)));
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            let result = serve(commands, forward, stopped, relay, inbox.clone()).await;
            if let Err(error) = &result {
                inbox.send(magistr_cockpit::cockpit::Msg::Agent(
                    magistr_application::sessions::AgentEvent::Error(error.clone()),
                ));
            }
            if let Some(observation) = observation {
                observation.stop();
            }
            feed.abort_and_join().await;
            reseed.abort_and_join().await;
            history_feed.abort_and_join().await;
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
                supervision,
                snapshot: seed.into(),
                workspace_files,
                ..ProjectSeed::default()
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

pub(super) struct Resources {
    pub(super) engines: EngineClient,
    admitted: std::sync::Arc<AdmittedResources>,
}
struct AdmittedResources {
    endpoint: Option<OwnerEndpoint>,
    workspace: WorkspaceAdmission,
}

impl Resources {
    pub(super) async fn open(
        engines: EngineClient,
        inputs: &crate::config::inputs::ProjectInputs,
    ) -> eyre::Result<Self> {
        Self::open_served(engines, inputs, true).await
    }

    pub(super) async fn open_served(
        engines: EngineClient,
        inputs: &crate::config::inputs::ProjectInputs,
        serve: bool,
    ) -> eyre::Result<Self> {
        engines
            .port()
            .ok_or_else(|| eyre::eyre!("Magistr could not connect to the project"))?;
        let endpoint = if serve {
            engines.serve_owner().await?
        } else {
            None
        };
        let workspace = WorkspaceAdmission::admit(inputs).await;
        Ok(Self {
            admitted: std::sync::Arc::new(AdmittedResources {
                endpoint,
                workspace,
            }),
            engines,
        })
    }

    pub(super) fn is_admitted(&self) -> bool {
        self.admitted.workspace.hosted().is_some()
    }
    pub(super) fn source_reader(
        &self,
    ) -> Option<std::sync::Weak<magistr_workspace::service::WorkspaceService>> {
        self.admitted
            .workspace
            .hosted()
            .map(|workspace| std::sync::Arc::downgrade(workspace.service()))
    }
    pub(super) fn source_report(&self) -> String {
        self.admitted.workspace.report()
    }

    pub(super) fn fork(&self, engines: EngineClient) -> Self {
        Self {
            engines,
            admitted: std::sync::Arc::clone(&self.admitted),
        }
    }

    pub(super) async fn close_last(self) -> Result<(), String> {
        if std::sync::Arc::strong_count(&self.admitted) != 1 {
            return Err("Session references remain after closing the working copy.".into());
        }
        self.close().await
    }

    pub(super) async fn close(self) -> Result<(), String> {
        let Self { engines, admitted } = self;
        let Ok(admitted) = std::sync::Arc::try_unwrap(admitted) else {
            drop(engines);
            return Ok(());
        };
        let endpoint = match admitted.endpoint {
            Some(endpoint) => endpoint.shutdown().await.map_err(|error| error.to_string()),
            None => Ok(()),
        };
        let workspace = admitted
            .workspace
            .shutdown()
            .await
            .map_err(|error| error.to_string());
        // The workflow writer is released only after its endpoint and workspace.

        drop(engines);
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
    forward: tokio::sync::mpsc::Sender<Cmd>,
    mut stopped: oneshot::Receiver<()>,
    mut relay: OwnedTask<Result<(), String>>,
    inbox: Inbox,
) -> Result<(), String> {
    loop {
        tokio::select! {
            biased;
            _ = &mut stopped => break,
            result = &mut relay.0 => {
                joined(result)?;
                return Err("The session ended. Reopen it to continue.".into());
            },
            command = commands.next() => {
                match command {
                    None | Some(Cmd::Quit) => break,
                    Some(command) => {
                        if let Err(error) = forward.try_send(command) {
                            let message = match &error {
                                tokio::sync::mpsc::error::TrySendError::Full(_) => "The session is busy. Try the action again.",
                                tokio::sync::mpsc::error::TrySendError::Closed(_) => "The session connection closed. Reopen the session and try again.",
                            };
                            inbox.send(magistr_cockpit::cockpit::Msg::CommandFailed { cmd: error.into_inner(), error: message.into() });
                        }
                    }
                }
            }
        }
    }
    // Quit overtakes queued commands on close: discard the external receiver.
    drop(commands);
    // Dropping the only sender closes the coordinator after its bounded queue.
    let _ = forward.try_send(Cmd::Quit);
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
