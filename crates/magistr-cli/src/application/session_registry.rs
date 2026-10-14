//! Application session admission, addressed controls and orderly settlement form
//! one ownership contract. Views hold subscriptions; this registry holds hosts.
use super::{
    InteractiveHost, SessionOpening,
    interactive_host::combine,
    session_observation::{SessionObservation, SessionSnapshot, SessionSubscription},
};
use magistr_cockpit::cockpit::{Cmd, ProjectSeed};
use magistr_cockpit_runtime::channel::{Commands, Inbox};
use magistr_domain::identity::SessionAddress;
use std::{
    collections::BTreeMap,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::{JoinHandle, JoinSet},
};

const CAPACITY: usize = 32;
const MAX_INPUT_BYTES: usize = 1024 * 1024;
const MAX_SESSIONS: usize = 64;
const CLOSE_GRACE: Duration = Duration::from_secs(8);
static INSTANCE: AtomicU64 = AtomicU64::new(1);

pub(super) trait Settle: Send + 'static {
    fn settle(self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
}
impl Settle for InteractiveHost {
    fn settle(self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        Box::pin(self.shutdown())
    }
}
pub(super) struct Opening {
    pub(super) address: SessionAddress,
    pub(super) choice: SessionOpening,
    pub(super) inbox: Inbox,
    pub(super) commands: Commands,
    pub(super) observation: Arc<SessionObservation>,
}
type Opened<H> = Pin<Box<dyn Future<Output = Result<(H, ProjectSeed), String>> + Send>>;
pub(super) type Open<H> = Box<dyn FnMut(Opening) -> Opened<H> + Send>;
pub(super) type Finish =
    Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> + Send>;

enum Request {
    Open {
        project: String,
        root: PathBuf,
        choice: SessionOpening,
        reply: oneshot::Sender<Result<SessionAddress, String>>,
    },
    Command {
        address: SessionAddress,
        command: Cmd,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Subscribe {
        address: SessionAddress,
        reply: oneshot::Sender<Result<SessionSubscription, String>>,
    },
    List {
        reply: oneshot::Sender<Vec<SessionSnapshot>>,
    },
    Draft {
        address: SessionAddress,
        text: String,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Close {
        address: SessionAddress,
        reply: oneshot::Sender<Result<(), String>>,
    },
}

/// Clonable control port. Every session action names its complete admitted address.
#[derive(Clone)]
pub struct SessionClient {
    sender: mpsc::Sender<Request>,
    catalog: watch::Receiver<super::SessionCatalog>,
}

impl SessionClient {
    /// Observe every resident session without subscribing to its transcript.
    #[must_use]
    pub fn catalog(&self) -> super::CatalogSubscription {
        super::CatalogSubscription {
            receiver: self.catalog.clone(),
        }
    }
    /// Resolve admission inputs away from the surface, then start an independent session.
    pub async fn open(
        &self,
        root: impl AsRef<Path>,
        choice: SessionOpening,
    ) -> Result<SessionAddress, String> {
        let root = root.as_ref().to_path_buf();
        let (project, root) = tokio::task::spawn_blocking(move || prepared(&root))
            .await
            .map_err(|error| error.to_string())??;
        let (reply, result) = oneshot::channel();
        self.post(Request::Open {
            project,
            root,
            choice,
            reply,
        })?;
        result
            .await
            .map_err(|_| "The application closed while opening the session.".to_owned())?
    }

    pub async fn command(&self, address: &SessionAddress, command: Cmd) -> Result<(), String> {
        if let Cmd::Prompt(text) = &command {
            if text.trim().is_empty() {
                return Err("Write a message before sending.".into());
            }
            check_input(text)?;
        }
        let (reply, result) = oneshot::channel();
        self.post(Request::Command {
            address: address.clone(),
            command,
            reply,
        })?;
        result
            .await
            .map_err(|_| "The application connection closed.".to_owned())?
    }

    pub async fn subscribe(&self, address: &SessionAddress) -> Result<SessionSubscription, String> {
        let (reply, result) = oneshot::channel();
        self.post(Request::Subscribe {
            address: address.clone(),
            reply,
        })?;
        let mut subscription = result
            .await
            .map_err(|_| "The application connection closed.".to_owned())??;
        subscription.refresh().await?;
        Ok(subscription)
    }

    pub async fn list(&self) -> Result<Vec<SessionSnapshot>, String> {
        let (reply, result) = oneshot::channel();
        self.post(Request::List { reply })?;
        result
            .await
            .map_err(|_| "The application connection closed.".to_owned())
    }

    /// Remove and settle precisely this session. Other sessions keep serving.
    pub async fn close(&self, address: &SessionAddress) -> Result<(), String> {
        let (reply, result) = oneshot::channel();
        self.post(Request::Close {
            address: address.clone(),
            reply,
        })?;
        result
            .await
            .map_err(|_| "The application connection closed.".to_owned())?
    }

    pub async fn draft(&self, address: &SessionAddress, text: String) -> Result<(), String> {
        check_input(&text)?;
        let (reply, result) = oneshot::channel();
        self.post(Request::Draft {
            address: address.clone(),
            text,
            reply,
        })?;
        result
            .await
            .map_err(|_| "The application connection closed.".to_owned())?
    }

    fn post(&self, request: Request) -> Result<(), String> {
        self.sender.try_send(request).map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => {
                "The application is busy. Try the action again.".into()
            }
            mpsc::error::TrySendError::Closed(_) => "The application connection closed.".into(),
        })
    }
}

fn check_input(text: &str) -> Result<(), String> {
    if text.len() > MAX_INPUT_BYTES {
        Err("This message is too long. Send a smaller part.".into())
    } else {
        Ok(())
    }
}

/// One application owner. Drop requests quit; shutdown awaits all owned resources.
pub struct SessionRegistry {
    client: SessionClient,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<(), String>>,
}

impl SessionRegistry {
    pub(super) fn start_with<H: Settle>(open: Open<H>, finish: Finish) -> Self {
        let (sender, requests) = mpsc::channel(CAPACITY);
        let (catalog, catalog_reader) = watch::channel(super::SessionCatalog::default());
        let (stop, stopped) = oneshot::channel();
        let instance = format!(
            "{}-{}-{}",
            std::process::id(),
            INSTANCE.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let task = tokio::spawn(
            Serving {
                open,
                finish,
                catalog,
                sessions: BTreeMap::new(),
                closing: BTreeMap::new(),
                pending: JoinSet::new(),
                instance,
                resume_claims: BTreeMap::new(),
                issued: 0,
                opening: 0,
                failures: Ok(()),
                wake: Arc::default(),
            }
            .run(requests, stopped),
        );
        Self {
            client: SessionClient {
                sender,
                catalog: catalog_reader,
            },
            stop: Some(stop),
            task,
        }
    }

    #[must_use]
    pub fn client(&self) -> SessionClient {
        self.client.clone()
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
impl Drop for SessionRegistry {
    fn drop(&mut self) {
        self.request_close();
    }
}

struct Resident<H> {
    host: H,
    commands: mpsc::Sender<Cmd>,
    observation: Arc<SessionObservation>,
}
enum Completed<H> {
    Opened {
        address: SessionAddress,
        observation: Arc<SessionObservation>,
        commands: mpsc::Sender<Cmd>,
        opened: Box<Result<(H, ProjectSeed), String>>,
        reply: oneshot::Sender<Result<SessionAddress, String>>,
    },
    Closed {
        address: SessionAddress,
        result: Result<(), String>,
    },
}
struct Serving<H> {
    catalog: watch::Sender<super::SessionCatalog>,
    open: Open<H>,
    finish: Finish,
    sessions: BTreeMap<SessionAddress, Resident<H>>,
    closing: BTreeMap<SessionAddress, Arc<SessionObservation>>,
    pending: JoinSet<Completed<H>>,
    instance: String,
    resume_claims: BTreeMap<SessionAddress, String>,
    issued: u64,
    opening: usize,
    failures: Result<(), String>,
    wake: Arc<tokio::sync::Notify>,
}
impl<H: Settle> Serving<H> {
    async fn run(
        mut self,
        mut requests: mpsc::Receiver<Request>,
        mut stopped: oneshot::Receiver<()>,
    ) -> Result<(), String> {
        loop {
            tokio::select! {
                biased;
                _ = &mut stopped => break,
                result = self.pending.join_next(), if !self.pending.is_empty() => {
                    if let Some(result) = result { self.completed(result, false); }
                }
                () = self.wake.notified() => self.retire_closed(),
                request = requests.recv() => match request {
                    Some(request) => self.request(request),
                    None => break,
                },
            }
            self.publish_catalog();
        }
        requests.close();
        drop(requests);
        for (address, resident) in std::mem::take(&mut self.sessions) {
            resident.observation.stop();
            self.pending.spawn(async move {
                Completed::Closed {
                    address,
                    result: resident.host.settle().await,
                }
            });
        }
        let drained = tokio::time::timeout(CLOSE_GRACE, async {
            while let Some(result) = self.pending.join_next().await {
                self.completed(result, true);
            }
        })
        .await;
        if drained.is_err() {
            self.pending.abort_all();
            while self.pending.join_next().await.is_some() {}
            self.failures = combine(
                self.failures,
                Err("Some sessions took too long to close and were stopped.".into()),
            );
        }
        drop(self.open);
        let cleanup = tokio::time::timeout(CLOSE_GRACE, (self.finish)())
            .await
            .unwrap_or_else(|_| {
                Err("Some working copies took too long to close and were stopped.".into())
            });
        combine(self.failures, cleanup)
    }

    fn publish_catalog(&self) {
        let sessions = self
            .sessions
            .values()
            .map(|resident| resident.observation.summary())
            .chain(
                self.closing
                    .values()
                    .map(|observation| observation.summary()),
            )
            .collect::<Vec<_>>();
        self.catalog.send_if_modified(|catalog| {
            if catalog.sessions == sessions {
                return false;
            }
            catalog.sessions = sessions;
            catalog.sequence = catalog.sequence.saturating_add(1);
            true
        });
    }

    fn retire_closed(&mut self) {
        use magistr_cockpit_runtime::channel::ObservationSink;
        let closed: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, resident)| resident.observation.is_closed())
            .map(|(address, _)| address.clone())
            .collect();
        for address in closed {
            if let Some(resident) = self.sessions.remove(&address) {
                self.closing
                    .insert(address.clone(), Arc::clone(&resident.observation));
                self.pending.spawn(async move {
                    Completed::Closed {
                        address,
                        result: resident.host.settle().await,
                    }
                });
            }
        }
    }

    fn request(&mut self, request: Request) {
        match request {
            Request::Open {
                project,
                root,
                choice,
                reply,
            } => {
                if self.sessions.len() + self.closing.len() + self.opening >= MAX_SESSIONS {
                    let _ = reply.send(Err("Close a session before opening more.".into()));
                    return;
                }
                if let SessionOpening::Resume { id } = &choice
                    && (self.resume_claims.values().any(|claimed| claimed == id)
                        || self
                            .sessions
                            .values()
                            .map(|resident| &resident.observation)
                            .chain(self.closing.values())
                            .any(|observation| {
                                observation
                                    .snapshot()
                                    .conversation
                                    .as_ref()
                                    .is_some_and(|conversation| &conversation.id == id)
                            }))
                {
                    let _ = reply.send(Err(
                        "This conversation is already open. Choose it from Sessions.".into(),
                    ));
                    return;
                }
                self.issued += 1;
                self.opening += 1;
                let address = SessionAddress {
                    project,
                    workspace: root.display().to_string(),
                    session: format!("{}-{}", self.instance, self.issued),
                };
                if let SessionOpening::Resume { id } = &choice {
                    self.resume_claims.insert(address.clone(), id.clone());
                }
                let observation = Arc::new(SessionObservation::new(address.clone()));
                observation.wake_with(Arc::clone(&self.wake));
                let (commands, receiver) = Commands::bounded(CAPACITY);
                let opened = (self.open)(Opening {
                    address: address.clone(),
                    choice,
                    inbox: Inbox::observed(observation.clone()),
                    commands: receiver,
                    observation: Arc::clone(&observation),
                });
                self.pending.spawn(async move {
                    let opened = opened.await;
                    Completed::Opened {
                        address,
                        observation,
                        commands,
                        opened: Box::new(opened),
                        reply,
                    }
                });
            }
            Request::Command {
                address,
                command,
                reply,
            } => {
                let rejected = command.clone();
                let result = self.command(&address, command);
                if let Err(error) = &result
                    && !matches!(rejected, Cmd::Permission { .. })
                    && let Some(resident) = self.sessions.get(&address)
                {
                    magistr_cockpit_runtime::channel::ObservationSink::send(
                        &*resident.observation,
                        magistr_cockpit::cockpit::Msg::CommandFailed {
                            cmd: rejected,
                            error: error.clone(),
                        },
                    );
                }
                let _ = reply.send(result);
            }
            Request::Subscribe { address, reply } => {
                let _ = reply.send(
                    self.sessions
                        .get(&address)
                        .map(|resident| resident.observation.subscribe())
                        .ok_or_else(gone),
                );
            }
            Request::List { reply } => {
                let _ = reply.send(
                    self.sessions
                        .values()
                        .map(|resident| resident.observation.snapshot())
                        .collect(),
                );
            }
            Request::Draft {
                address,
                text,
                reply,
            } => {
                let result = self
                    .sessions
                    .get(&address)
                    .map(|resident| resident.observation.draft(text))
                    .ok_or_else(gone);
                let _ = reply.send(result);
            }
            Request::Close { address, reply } => match self.sessions.remove(&address) {
                Some(resident) => {
                    resident.observation.stop();
                    self.closing
                        .insert(address.clone(), Arc::clone(&resident.observation));
                    self.pending.spawn(async move {
                        let result = tokio::time::timeout(CLOSE_GRACE, resident.host.settle())
                            .await
                            .unwrap_or_else(|_| {
                                Err("The session took too long to close and was stopped.".into())
                            });
                        let _ = reply.send(result.clone());
                        Completed::Closed { address, result }
                    });
                }
                None => {
                    let _ = reply.send(Err(gone()));
                }
            },
        }
    }

    fn command(&self, address: &SessionAddress, command: Cmd) -> Result<(), String> {
        if matches!(command, Cmd::Quit) {
            return Err("Choose a session action or close the addressed session.".into());
        }
        let resident = self.sessions.get(address).ok_or_else(gone)?;
        if let Cmd::Permission { id, ref option_id } = command {
            if resident.observation.is_answered(id) {
                return Err("This request has already been answered.".into());
            }
            let snapshot = resident.observation.snapshot();
            if snapshot.closed
                || snapshot
                    .activity
                    .as_ref()
                    .is_some_and(|activity| activity.cancellation_requested)
            {
                return Err("This request is no longer waiting for an answer.".into());
            }
            let Some(magistr_cockpit::cockpit::Msg::Agent(
                magistr_application::sessions::AgentEvent::PermissionRequest { choices, .. },
            )) = snapshot.pending.get(&id)
            else {
                return Err("This request is no longer waiting for an answer.".into());
            };
            if !choices.iter().any(|choice| choice.option_id == *option_id) {
                return Err("This answer was not offered by the request.".into());
            }
        }
        let answered = if let Cmd::Permission { id, .. } = command {
            Some(id)
        } else {
            None
        };
        resident
            .commands
            .try_send(command)
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => {
                    "The session is busy. Try the action again.".to_owned()
                }
                mpsc::error::TrySendError::Closed(_) => {
                    "The agent connection closed. Reopen the session.".to_owned()
                }
            })?;
        if let Some(id) = answered {
            resident.observation.permission_answered(id);
        }
        Ok(())
    }

    fn completed(&mut self, result: Result<Completed<H>, tokio::task::JoinError>, quitting: bool) {
        match result {
            Ok(Completed::Opened {
                address,
                observation,
                commands,
                opened,
                reply,
            }) => {
                self.opening = self.opening.saturating_sub(1);
                match *opened {
                    Ok((host, mut seed)) => {
                        seed.session_address = Some(address.clone());
                        observation.seed(seed);
                        if quitting {
                            observation.stop();
                            self.pending.spawn(async move {
                                Completed::Closed {
                                    address,
                                    result: host.settle().await,
                                }
                            });
                            let _ = reply.send(Err("The application is closing.".into()));
                        } else {
                            self.sessions.insert(
                                address.clone(),
                                Resident {
                                    host,
                                    commands,
                                    observation,
                                },
                            );
                            let _ = reply.send(Ok(address));
                        }
                    }
                    Err(error) => {
                        self.resume_claims.remove(&address);
                        observation.stop();
                        let _ = reply.send(Err(error));
                    }
                }
            }
            Ok(Completed::Closed { address, result }) => {
                self.closing.remove(&address);
                self.resume_claims.remove(&address);
                self.failures = combine(std::mem::replace(&mut self.failures, Ok(())), result);
            }
            Err(error) => {
                self.failures = combine(
                    std::mem::replace(&mut self.failures, Ok(())),
                    Err(format!("A session worker failed: {error}")),
                );
            }
        }
    }
}
fn gone() -> String {
    "This session is no longer open. Select an open session and try again.".into()
}

fn prepared(dir: &Path) -> Result<(String, PathBuf), String> {
    let folder = super::ProjectFolder::inspect(dir).map_err(|error| error.to_string())?;
    let root = folder.directory;
    if !folder.prepared {
        return Err(format!(
            "{} needs Magistr setup. Review setup in the desktop or run `magistr init` in that folder.",
            root.display()
        ));
    }
    let config = magistr_core::evidence::writer_config_from_env(&root);
    Ok((config.resolve_project_id(), root))
}

#[cfg(test)]
#[path = "session_registry.tests.rs"]
pub(super) mod tests;
