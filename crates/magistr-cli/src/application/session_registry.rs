//! Application session admission, addressed controls and orderly settlement form
//! one ownership contract. Views hold subscriptions; this registry holds hosts.
use super::session_attachments::{AttachmentSelection, matches_ids, prepare_send};
use super::{
    InteractiveHost, SessionOpening,
    interactive_host::combine,
    session_observation::{SessionObservation, SessionSnapshot, SessionSubscription},
};
use magistr_application::sessions::{MAX_PROMPT_BYTES, PromptInput};
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
const MAX_INPUT_BYTES: usize = MAX_PROMPT_BYTES;
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
        address: ConversationTarget,
        command: Cmd,
        reply: oneshot::Sender<Result<(), String>>,
    },
    ControlWork {
        target: super::WorkTarget,
        action: magistr_cockpit::cockpit::LoopCmd,
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
        address: ConversationTarget,
        expected_revision: u64,
        text: String,
        discard: bool,
        reply: oneshot::Sender<Result<super::DraftStatus, String>>,
    },
    SavedDraft {
        inputs: Box<crate::config::inputs::ProjectInputs>,
        id: String,
        expected_revision: u64,
        edit: crate::agent::persistence::draft::SavedEdit,
        reply: oneshot::Sender<Result<super::DraftStatus, String>>,
    },
    Attach {
        address: ConversationTarget,
        selection: AttachmentSelection,
        reply: oneshot::Sender<Result<(), String>>,
    },
    RemoveAttachment {
        address: ConversationTarget,
        id: String,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Send {
        address: ConversationTarget,
        text: String,
        ids: Vec<String>,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Close {
        address: SessionAddress,
        reply: oneshot::Sender<Result<(), String>>,
    },
}

/// One logical conversation within a resident application session.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ConversationTarget {
    pub address: SessionAddress,
    pub conversation: String,
}

/// Clonable control port. Conversation actions name their exact retained owner.
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

    pub async fn command(&self, address: &ConversationTarget, command: Cmd) -> Result<(), String> {
        if let Cmd::Prompt(input) | Cmd::RoadmapPrompt(input) = &command {
            input.validate(true)?;
            if input.is_empty() {
                return Err("Write a message or attach a file before sending.".into());
            }
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

    /// Queue a control for this run; the executing owner validates it again.
    pub async fn control_work(
        &self,
        target: &super::WorkTarget,
        action: magistr_cockpit::cockpit::LoopCmd,
    ) -> Result<(), String> {
        let (reply, result) = oneshot::channel();
        self.post(Request::ControlWork {
            target: target.clone(),
            action,
            reply,
        })?;
        result.await.map_err(|_| gone())?
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

    pub async fn draft(
        &self,
        address: &ConversationTarget,
        expected_revision: u64,
        text: String,
    ) -> Result<super::DraftStatus, String> {
        check_input(&text)?;
        self.edit_draft(address, expected_revision, text, false)
            .await
    }

    pub async fn discard_draft(
        &self,
        address: &ConversationTarget,
        expected_revision: u64,
    ) -> Result<super::DraftStatus, String> {
        self.edit_draft(address, expected_revision, String::new(), true)
            .await
    }

    async fn edit_draft(
        &self,
        address: &ConversationTarget,
        expected_revision: u64,
        text: String,
        discard: bool,
    ) -> Result<super::DraftStatus, String> {
        let (reply, result) = oneshot::channel();
        self.post(Request::Draft {
            address: address.clone(),
            expected_revision,
            text,
            discard,
            reply,
        })?;
        result
            .await
            .map_err(|_| "The application connection closed.".to_owned())?
    }

    pub async fn save_saved_draft(
        &self,
        inputs: crate::config::inputs::ProjectInputs,
        id: String,
        expected_revision: u64,
        text: String,
    ) -> Result<super::DraftStatus, String> {
        check_input(&text)?;
        self.edit_saved_draft(
            inputs,
            id,
            expected_revision,
            crate::agent::persistence::draft::SavedEdit::Text(text),
        )
        .await
    }

    pub async fn discard_saved_draft(
        &self,
        inputs: crate::config::inputs::ProjectInputs,
        id: String,
        expected_revision: u64,
    ) -> Result<super::DraftStatus, String> {
        self.edit_saved_draft(
            inputs,
            id,
            expected_revision,
            crate::agent::persistence::draft::SavedEdit::Discard,
        )
        .await
    }

    pub async fn remove_saved_attachment(
        &self,
        inputs: crate::config::inputs::ProjectInputs,
        id: String,
        expected_revision: u64,
        attachment_id: String,
    ) -> Result<super::DraftStatus, String> {
        self.edit_saved_draft(
            inputs,
            id,
            expected_revision,
            crate::agent::persistence::draft::SavedEdit::Remove(attachment_id),
        )
        .await
    }

    async fn edit_saved_draft(
        &self,
        inputs: crate::config::inputs::ProjectInputs,
        id: String,
        expected_revision: u64,
        edit: crate::agent::persistence::draft::SavedEdit,
    ) -> Result<super::DraftStatus, String> {
        let (reply, result) = oneshot::channel();
        self.post(Request::SavedDraft {
            inputs: Box::new(inputs),
            id,
            expected_revision,
            edit,
            reply,
        })?;
        result
            .await
            .map_err(|_| "The application connection closed.".to_owned())?
    }

    /// Stage native file selections only through this session's admitted workspace.
    pub async fn attach_files(
        &self,
        address: &ConversationTarget,
        paths: &[PathBuf],
    ) -> Result<(), String> {
        self.attach(address, AttachmentSelection::Files(paths.to_vec()))
            .await
    }

    /// Paste user-provided bytes; this carries no filesystem path or read capability.
    pub async fn attach_bytes(
        &self,
        address: &ConversationTarget,
        name: String,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        if bytes.len() > magistr_application::sessions::MAX_ATTACHMENT_BYTES {
            return Err("Choose a file of 1 MiB or less.".into());
        }
        self.attach(address, AttachmentSelection::Bytes { name, bytes })
            .await
    }

    async fn attach(
        &self,
        address: &ConversationTarget,
        selection: AttachmentSelection,
    ) -> Result<(), String> {
        let (reply, result) = oneshot::channel();
        self.post(Request::Attach {
            address: address.clone(),
            selection,
            reply,
        })?;
        result
            .await
            .map_err(|_| "The application connection closed.".to_owned())?
    }

    pub async fn remove_attachment(
        &self,
        address: &ConversationTarget,
        id: String,
    ) -> Result<(), String> {
        let (reply, result) = oneshot::channel();
        self.post(Request::RemoveAttachment {
            address: address.clone(),
            id,
            reply,
        })?;
        result
            .await
            .map_err(|_| "The application connection closed.".to_owned())?
    }

    /// Validate selected source revisions, then queue the exact immutable message.
    pub async fn send(
        &self,
        address: &ConversationTarget,
        text: String,
        attachment_ids: Vec<String>,
    ) -> Result<(), String> {
        check_input(&text)?;
        let (reply, result) = oneshot::channel();
        self.post(Request::Send {
            address: address.clone(),
            text,
            ids: attachment_ids,
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
                saved_writes: std::collections::BTreeSet::new(),
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
    send_generation: u64,
    preparing: Option<String>,
}
enum Completed<H> {
    DraftSaved {
        id: Option<String>,
        result: Result<super::DraftStatus, String>,
        reply: oneshot::Sender<Result<super::DraftStatus, String>>,
    },
    MutationSaved {
        result: Result<(), String>,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Attached {
        address: ConversationTarget,
        result: Result<Vec<super::StagedAttachment>, String>,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Prepared {
        address: ConversationTarget,
        generation: u64,
        ids: Vec<String>,
        input: Result<PromptInput, String>,
        reply: oneshot::Sender<Result<(), String>>,
    },
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
    saved_writes: std::collections::BTreeSet<String>,
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
                    result: settle_resident(resident).await,
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
                        result: settle_resident(resident).await,
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
                    && (self.saved_writes.contains(id)
                        || self.resume_claims.values().any(|claimed| claimed == id)
                        || self
                            .sessions
                            .values()
                            .map(|resident| &resident.observation)
                            .chain(self.closing.values())
                            .any(|observation| observation.owns(id)))
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
                    && let Some(resident) = self.sessions.get(&address.address)
                {
                    magistr_cockpit_runtime::channel::ObservationSink::send_for(
                        &*resident.observation,
                        &address.conversation,
                        magistr_cockpit::cockpit::Msg::CommandFailed {
                            cmd: rejected,
                            error: error.clone(),
                        },
                    );
                }
                let _ = reply.send(result);
            }
            Request::ControlWork {
                target,
                action,
                reply,
            } => {
                let _ = reply.send(self.control_work(&target, action));
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
                expected_revision,
                text,
                discard,
                reply,
            } => self.edit_resident_draft(address, expected_revision, text, discard, reply),
            Request::SavedDraft {
                inputs,
                id,
                expected_revision,
                edit,
                reply,
            } => self.edit_saved_draft(*inputs, id, expected_revision, edit, reply),
            Request::Attach {
                address,
                selection,
                reply,
            } => self.attach(address, selection, reply),
            Request::RemoveAttachment { address, id, reply } => {
                self.remove_pending_attachment(address, id, reply)
            }
            Request::Send {
                address,
                text,
                ids,
                reply,
            } => self.prepare_submission(address, text, ids, reply),
            Request::Close { address, reply } => match self.sessions.remove(&address) {
                Some(resident) => {
                    resident.observation.stop();
                    self.closing
                        .insert(address.clone(), Arc::clone(&resident.observation));
                    self.pending.spawn(async move {
                        let result = tokio::time::timeout(CLOSE_GRACE, settle_resident(resident))
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

    fn edit_resident_draft(
        &mut self,
        address: ConversationTarget,
        expected_revision: u64,
        text: String,
        discard: bool,
        reply: oneshot::Sender<Result<super::DraftStatus, String>>,
    ) {
        if self.pending.len() >= CAPACITY {
            let _ = reply.send(Err(
                "The application is busy. Try saving your message again.".into(),
            ));
            return;
        }
        let observation = self
            .sessions
            .get(&address.address)
            .map(|resident| Arc::clone(&resident.observation))
            .ok_or_else(gone);
        match observation.and_then(|observation| {
            observation.validate_target(&address)?;
            let revision = observation.edit_draft_for(
                &address.conversation,
                expected_revision,
                text,
                discard,
            )?;
            Ok((observation, revision))
        }) {
            Ok((observation, revision)) => {
                self.pending.spawn(async move {
                    let mut status = observation.flush_draft_for(&address.conversation).await;
                    status.revision = revision;
                    Completed::DraftSaved {
                        id: None,
                        result: Ok(status),
                        reply,
                    }
                });
            }
            Err(error) => {
                let _ = reply.send(Err(error));
            }
        }
    }

    fn edit_saved_draft(
        &mut self,
        inputs: crate::config::inputs::ProjectInputs,
        id: String,
        expected_revision: u64,
        edit: crate::agent::persistence::draft::SavedEdit,
        reply: oneshot::Sender<Result<super::DraftStatus, String>>,
    ) {
        let resident = self.resume_claims.values().any(|claimed| claimed == &id)
            || self
                .sessions
                .values()
                .map(|resident| &resident.observation)
                .chain(self.closing.values())
                .any(|observation| observation.owns(&id));
        if self.pending.len() >= CAPACITY {
            let _ = reply.send(Err(
                "The application is busy. Try saving your message again.".into(),
            ));
        } else if resident || self.saved_writes.contains(&id) {
            let _ = reply.send(Err("This conversation is open or still saving. Use its open message, or wait before editing the saved message.".into()));
        } else {
            self.saved_writes.insert(id.clone());
            self.pending.spawn(async move {
                let saved_id = id.clone();
                let result = tokio::task::spawn_blocking(move || {
                    crate::agent::persistence::draft::edit_saved(
                        &inputs,
                        &id,
                        expected_revision,
                        edit,
                    )
                })
                .await
                .map_err(crate::agent::persistence::draft::save_failure)
                .and_then(|result| result);
                Completed::DraftSaved {
                    id: Some(saved_id),
                    result,
                    reply,
                }
            });
        }
    }

    fn remove_pending_attachment(
        &mut self,
        address: ConversationTarget,
        id: String,
        reply: oneshot::Sender<Result<(), String>>,
    ) {
        let observation = self
            .sessions
            .get(&address.address)
            .map(|resident| Arc::clone(&resident.observation))
            .ok_or_else(gone);
        match observation.and_then(|observation| {
            observation.validate_target(&address)?;
            observation.remove_attachment_for(&address.conversation, &id)?;
            Ok(observation)
        }) {
            Ok(observation) => self.await_mutation(observation, address.conversation, reply),
            Err(error) => {
                let _ = reply.send(Err(error));
            }
        }
    }

    fn await_mutation(
        &mut self,
        observation: Arc<SessionObservation>,
        conversation: String,
        reply: oneshot::Sender<Result<(), String>>,
    ) {
        self.pending.spawn(async move {
            let status = observation.flush_draft_for(&conversation).await;
            Completed::MutationSaved {
                result: status.error.map_or(Ok(()), Err),
                reply,
            }
        });
    }

    fn attach(
        &mut self,
        address: ConversationTarget,
        selection: AttachmentSelection,
        reply: oneshot::Sender<Result<(), String>>,
    ) {
        let Some(resident) = self.sessions.get(&address.address) else {
            let _ = reply.send(Err(gone()));
            return;
        };
        if let Err(error) = resident.observation.validate_target(&address) {
            let _ = reply.send(Err(error));
            return;
        }
        if self.pending.len() >= CAPACITY {
            let _ = reply.send(Err("The application is busy. Try the action again.".into()));
            return;
        }
        let source = resident.observation.source();
        self.pending.spawn(async move {
            let result = tokio::task::spawn_blocking(move || selection.prepare(source.as_deref()))
                .await
                .map_err(|error| format!("The files could not be read: {error}"))
                .and_then(|result| result);
            Completed::Attached {
                address,
                result,
                reply,
            }
        });
    }

    fn prepare_submission(
        &mut self,
        address: ConversationTarget,
        text: String,
        ids: Vec<String>,
        reply: oneshot::Sender<Result<(), String>>,
    ) {
        let Some(resident) = self.sessions.get_mut(&address.address) else {
            let _ = reply.send(Err(gone()));
            return;
        };
        if let Err(error) = resident.observation.validate_active(&address) {
            let _ = reply.send(Err(error));
            return;
        }
        if resident.preparing.as_deref() == Some(&address.conversation) {
            let _ = reply.send(Err(
                "Your message is still being checked. Wait before sending again.".into(),
            ));
            return;
        }
        let Some(snapshot) = resident.observation.context_snapshot(&address.conversation) else {
            let _ = reply.send(Err(gone()));
            return;
        };
        if !matches_ids(&snapshot.attachments, &ids) {
            let _ = reply.send(Err(
                "The attachments changed. Review the message and send again.".into(),
            ));
            return;
        }
        if self.pending.len() >= CAPACITY {
            let _ = reply.send(Err("The application is busy. Try the action again.".into()));
            return;
        }
        if let Err(error) = resident
            .observation
            .draft_for(&address.conversation, text.clone())
        {
            let _ = reply.send(Err(error));
            return;
        }
        resident.send_generation += 1;
        resident.preparing = Some(address.conversation.clone());
        let generation = resident.send_generation;
        let source = resident.observation.source();
        self.pending.spawn(async move {
            let input = tokio::task::spawn_blocking(move || {
                prepare_send(text, &snapshot.attachments, source.as_deref())
            })
            .await
            .map_err(|error| format!("The files could not be checked: {error}"))
            .and_then(|result| result);
            Completed::Prepared {
                address,
                generation,
                ids,
                input,
                reply,
            }
        });
    }

    fn admit_prepared(
        &mut self,
        address: &ConversationTarget,
        generation: u64,
        ids: &[String],
        input: Result<PromptInput, String>,
    ) -> Result<(), String> {
        let resident = self.sessions.get_mut(&address.address).ok_or_else(gone)?;
        if resident.send_generation != generation
            || resident.preparing.as_deref() != Some(&address.conversation)
        {
            return Err("This send was stopped. Your draft is still available.".into());
        }
        resident.preparing = None;
        resident.observation.validate_active(address)?;
        let input = input?;
        let snapshot = resident
            .observation
            .context_snapshot(&address.conversation)
            .ok_or_else(gone)?;
        if !matches_ids(&snapshot.attachments, ids) {
            return Err("The attachments changed. Review the message and send again.".into());
        }
        input.validate(
            snapshot
                .activity
                .as_ref()
                .is_some_and(|activity| activity.can_send_images == Some(true)),
        )?;
        self.command(address, Cmd::Prompt(input))
    }

    fn control_work(
        &mut self,
        target: &super::WorkTarget,
        action: magistr_cockpit::cockpit::LoopCmd,
    ) -> Result<(), String> {
        let resident = self.sessions.get(&target.address).ok_or_else(gone)?;
        let snapshot = resident.observation.snapshot();
        if snapshot.closed
            || !snapshot
                .work
                .as_ref()
                .is_some_and(|work| work.id == target.run_id && work.is_active())
        {
            return Err(
                "That work run has ended. Review the current work before trying again.".into(),
            );
        }
        resident
            .commands
            .try_send(Cmd::Loop(magistr_cockpit::cockpit::RunCommand {
                run_id: target.run_id.clone(),
                action,
            }))
            .map_err(|_| "The session is busy or closed. Try the action again.".to_owned())
    }

    fn command(&mut self, address: &ConversationTarget, command: Cmd) -> Result<(), String> {
        if matches!(command, Cmd::Loop(_)) {
            return Err("Work controls must name the current work run.".into());
        }
        if let Cmd::Prompt(input) | Cmd::RoadmapPrompt(input) = &command {
            input.validate(true)?;
            if input.is_empty() {
                return Err("Write a message or attach a file before sending.".into());
            }
        }
        if matches!(command, Cmd::Quit) {
            return Err("Choose a session action or close the addressed session.".into());
        }
        let resident = self.sessions.get_mut(&address.address).ok_or_else(gone)?;
        resident.observation.validate_active(address)?;
        if resident.preparing.as_deref() == Some(&address.conversation)
            && matches!(command, Cmd::Prompt(_) | Cmd::RoadmapPrompt(_))
        {
            return Err("Your message is still being checked. Wait before sending again.".into());
        }
        let cancel = matches!(command, Cmd::Cancel);
        if let Cmd::Permission { id, ref option_id } = command {
            if resident.observation.is_answered(&address.conversation, id) {
                return Err("This request has already been answered.".into());
            }
            let snapshot = resident
                .observation
                .context_snapshot(&address.conversation)
                .ok_or_else(gone)?;
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
        let permit = resident
            .commands
            .try_reserve()
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => {
                    "The session is busy. Try the action again.".to_owned()
                }
                mpsc::error::TrySendError::Closed(_) => {
                    "The agent connection closed. Reopen the session.".to_owned()
                }
            })?;
        if let Some(id) = answered {
            resident
                .observation
                .permission_answered_for(&address.conversation, id)?;
        }
        permit.send(Cmd::ForConversation {
            id: address.conversation.clone(),
            command: Box::new(command),
        });
        if cancel {
            resident.send_generation += 1;
            resident.preparing = None;
        }
        Ok(())
    }

    fn completed(&mut self, result: Result<Completed<H>, tokio::task::JoinError>, quitting: bool) {
        match result {
            Ok(Completed::DraftSaved { id, result, reply }) => {
                if let Some(id) = id {
                    self.saved_writes.remove(&id);
                }
                let _ = reply.send(result);
            }
            Ok(Completed::MutationSaved { result, reply }) => {
                let _ = reply.send(result);
            }
            Ok(Completed::Attached {
                address,
                result,
                reply,
            }) => {
                let result = result.and_then(|attachments| {
                    if quitting {
                        return Err("The application is closing.".into());
                    }
                    let observation = Arc::clone(
                        &self
                            .sessions
                            .get(&address.address)
                            .ok_or_else(gone)?
                            .observation,
                    );
                    observation.validate_target(&address)?;
                    observation.add_attachments_for(&address.conversation, attachments)?;
                    Ok(observation)
                });
                match result {
                    Ok(observation) => {
                        self.await_mutation(observation, address.conversation, reply)
                    }
                    Err(error) => {
                        let _ = reply.send(Err(error));
                    }
                }
            }
            Ok(Completed::Prepared {
                address,
                generation,
                ids,
                input,
                reply,
            }) => {
                let result = if quitting {
                    Err("The application is closing.".into())
                } else {
                    self.admit_prepared(&address, generation, &ids, input)
                };
                let _ = reply.send(result);
            }
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
                        if quitting || observation.snapshot().closed {
                            observation.stop();
                            self.pending.spawn(async move {
                                Completed::Closed {
                                    address,
                                    result: settle_host(host, observation).await,
                                }
                            });
                            let reason = if quitting {
                                "The application is closing."
                            } else {
                                "The session ended while opening. Try opening it again."
                            };
                            let _ = reply.send(Err(reason.into()));
                        } else {
                            self.sessions.insert(
                                address.clone(),
                                Resident {
                                    host,
                                    commands,
                                    observation,
                                    send_generation: 0,
                                    preparing: None,
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
async fn settle_resident<H: Settle>(resident: Resident<H>) -> Result<(), String> {
    settle_host(resident.host, resident.observation).await
}

async fn settle_host<H: Settle>(
    host: H,
    observation: Arc<SessionObservation>,
) -> Result<(), String> {
    let draft = observation.flush_all().await;
    combine(draft, host.settle().await)
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

#[cfg(test)]
#[path = "session_registry.ownership.tests.rs"]
mod ownership_tests;
