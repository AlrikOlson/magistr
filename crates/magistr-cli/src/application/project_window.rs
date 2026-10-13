//! Presentation adapter over the application's resident session registry.
//! Selecting a project subscribes to its session; only explicit close or app quit
//! settles hosts. Commands were addressed by the model before they entered a queue.
use super::{SessionClient, SessionRegistry, SessionSnapshot, SessionSubscription};
use crate::agent::AgentConfig;
use color_eyre::eyre;
use magistr_cockpit::cockpit::{Cmd, Model, Msg, ProjectSeed};
use magistr_cockpit_runtime::channel::{Commands, Inbox};
use magistr_domain::identity::SessionAddress;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{sync::oneshot, task::JoinHandle};

pub struct ProjectWindow {
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<(), String>>,
}
impl ProjectWindow {
    pub async fn start(
        first: AgentConfig,
        configure: impl Fn(&Path) -> eyre::Result<AgentConfig> + Send + Sync + 'static,
        model: Model,
        inbox: Inbox,
        commands: Commands,
    ) -> eyre::Result<(Self, Model)> {
        let root = first.cwd.clone();
        let registry = SessionRegistry::start(first, configure);
        let (window, seed) = Self::start_with(registry, root, inbox, commands)
            .await
            .map_err(|error| eyre::eyre!(error))?;
        Ok((window, seed.seed(model)))
    }

    pub(super) async fn start_with(
        registry: SessionRegistry,
        root: PathBuf,
        inbox: Inbox,
        commands: Commands,
    ) -> Result<(Self, ProjectSeed), String> {
        let client = registry.client();
        let address = match client.open(root).await {
            Ok(address) => address,
            Err(error) => {
                let cleanup = registry.shutdown().await;
                return Err(match cleanup {
                    Ok(()) => error,
                    Err(cleanup) => format!("{error}; {cleanup}"),
                });
            }
        };
        let mut subscription = client.subscribe(&address).await?;
        let snapshot = subscription.snapshot();
        let seeded = seed(&snapshot);
        let (stop, stopped) = oneshot::channel();
        let serving = Serving {
            client,
            inbox,
            selected: address,
            subscription,
            seen: snapshot.sequence,
            pending: None,
        };
        let task = tokio::spawn(async move {
            serving.run(commands, stopped).await;
            registry.shutdown().await
        });
        Ok((
            Self {
                stop: Some(stop),
                task,
            },
            seeded,
        ))
    }

    pub async fn shutdown(mut self) -> Result<(), String> {
        self.request_close();
        (&mut self.task)
            .await
            .map_err(|error| format!("The sessions could not close: {error}"))?
    }
    fn request_close(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}
impl Drop for ProjectWindow {
    fn drop(&mut self) {
        self.request_close();
    }
}

struct Serving {
    client: SessionClient,
    inbox: Inbox,
    selected: SessionAddress,
    subscription: SessionSubscription,
    seen: u64,
    pending: Option<SessionSnapshot>,
}
impl Serving {
    async fn run(mut self, mut commands: Commands, mut stopped: oneshot::Receiver<()>) {
        let mut tick = tokio::time::interval(Duration::from_millis(16));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                biased;
                _ = &mut stopped => break,
                command = commands.next() => match command {
                    None | Some(Cmd::Quit) => break,
                    Some(command) => {
                        tokio::select! {
                            _ = &mut stopped => break,
                            () = self.command(command) => {}
                        }

                    }
                },
                snapshot = self.subscription.changed() => match snapshot {
                    Some(snapshot) => self.pending = Some(snapshot),
                    None => break,
                },
                _ = tick.tick(), if self.pending.is_some() => {
                    if self.inbox.is_closed() { break; }
                    self.publish().await;
                }
            }
        }
    }

    async fn publish(&mut self) {
        if self
            .pending
            .as_ref()
            .is_some_and(|snapshot| snapshot.sequence == self.seen)
        {
            self.pending = None;
            return;
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|snapshot| snapshot.since(self.seen).is_none())
        {
            match self.subscription.refresh().await {
                Ok(snapshot) => self.pending = Some(snapshot),
                Err(error) => {
                    tracing::warn!(%error, "session reseed failed");
                    return;
                }
            }
        }
        let Some(snapshot) = self.pending.as_ref() else {
            return;
        };
        let (reseed, messages) = match snapshot.since(self.seen) {
            Some(messages) => (None, messages),
            None => (Some(Box::new(seed(snapshot))), Vec::new()),
        };
        if self.inbox.send(Msg::SessionFrame {
            address: snapshot.address.clone(),
            sequence: snapshot.sequence,
            seed: reseed,
            messages,
        }) {
            self.seen = snapshot.sequence;
            self.pending = None;
        }
    }

    async fn command(&mut self, command: Cmd) {
        let Cmd::Session {
            address,
            command,
            draft,
        } = command
        else {
            self.inbox.send(Msg::CommandFailed {
                cmd: command,
                error: "The action did not name an open session.".into(),
            });
            return;
        };
        let command = *command;
        if matches!(command, Cmd::OpenProject(_)) && address != self.selected {
            self.failed(
                address,
                command,
                "The view changed before this was opened. Try again.".into(),
            );
            return;
        }
        // Selection remains available after the viewed session closes. A removed
        // session no longer accepts draft writes, but cannot own navigation.
        let saved = match (&command, self.subscription.is_closed()) {
            (Cmd::OpenProject(_), true) => Ok(()),
            _ => self.client.draft(&address, draft).await,
        };
        if let Err(error) = saved {
            self.failed(address, command, error);
            return;
        }
        match command {
            Cmd::OpenProject(dir) => self.switch(dir).await,
            command => {
                if let Err(error) = self.client.command(&address, command.clone()).await {
                    self.failed(address, command, error);
                }
            }
        }
    }

    fn failed(&self, address: SessionAddress, command: Cmd, error: String) {
        self.inbox.send(Msg::CommandFailed {
            cmd: Cmd::Session {
                address,
                command: Box::new(command),
                draft: String::new(),
            },
            error,
        });
    }

    async fn switch(&mut self, dir: String) {
        let opened = async {
            let root = PathBuf::from(&dir);
            let canonical = tokio::task::spawn_blocking(move || root.canonicalize())
                .await
                .map_err(|error| error.to_string())?
                .map_err(|error| error.to_string())?;
            let listed = self.client.list().await?;
            let address = match listed
                .iter()
                .find(|snapshot| Path::new(&snapshot.address.workspace) == canonical)
            {
                Some(snapshot) => snapshot.address.clone(),
                None => self.client.open(canonical).await?,
            };
            let mut subscription = self.client.subscribe(&address).await?;
            let snapshot = subscription.snapshot();
            Ok::<_, String>((address, subscription, snapshot))
        }
        .await;
        match opened {
            Ok((address, subscription, snapshot)) => {
                self.selected = address;
                self.subscription = subscription;
                self.seen = snapshot.sequence;
                self.pending = Some(snapshot.clone());
                if self.inbox.send(Msg::SessionFrame {
                    address: snapshot.address.clone(),
                    sequence: snapshot.sequence,
                    seed: Some(Box::new(seed(&snapshot))),
                    messages: Vec::new(),
                }) {
                    self.pending = None;
                } else {
                    self.seen = u64::MAX;
                }
            }
            Err(error) => {
                self.inbox.send(Msg::CommandFailed {
                    cmd: Cmd::OpenProject(dir),
                    error,
                });
            }
        }
    }
}
fn seed(snapshot: &SessionSnapshot) -> ProjectSeed {
    let mut seed = snapshot.seed.clone();
    seed.replay = snapshot.replay();
    seed
}
#[cfg(test)]
#[path = "project_window.tests.rs"]
mod tests;
