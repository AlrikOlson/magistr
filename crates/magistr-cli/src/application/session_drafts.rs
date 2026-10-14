//! Draft persistence is an application service over the conversation history
//! owner. Views edit through SessionClient and observe actual durable revisions.
use super::{SessionSnapshot, StagedAttachment};
use crate::{agent::persistence::SessionHistory, config::inputs::ProjectInputs};
use serde::Serialize;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::watch;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct DraftStatus {
    pub revision: u64,
    pub saved_revision: u64,
    /// Latest completed save attempt within this resident owner's lifetime.
    /// Durable history reads and edits have no resident writer and report zero.
    pub save_attempt: u64,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SavedDraft {
    pub text: String,
    pub attachments: Vec<StagedAttachment>,
    pub status: DraftStatus,
}

impl SavedDraft {
    pub(super) fn from_snapshot(snapshot: &SessionSnapshot) -> Self {
        Self {
            text: snapshot.seed.draft.clone().unwrap_or_default(),
            attachments: snapshot.attachments.clone(),
            status: snapshot.draft_status(),
        }
    }
}

/// Reading a saved draft neither launches an agent nor creates a database.
pub fn saved_draft(inputs: &ProjectInputs, id: &str) -> Result<SavedDraft, String> {
    use crate::agent::persistence::{draft, storage};
    let path = storage::sessions_db_path(inputs)
        .ok_or_else(|| "No folder is available for saved conversations.".to_owned())?;
    let conn = storage::open_readonly_db(&path).map_err(draft::read_failure)?;
    if storage::get_session(&conn, id)
        .map_err(draft::read_failure)?
        .is_none()
    {
        return Err("This saved conversation is no longer available.".into());
    }
    draft::load(&conn, id)
}

#[derive(Clone, Debug)]
struct Pending {
    attempt: u64,
    draft: SavedDraft,
}
/// A watch channel coalesces pending edits while exactly one blocking SQLite
/// transaction runs. Failed writes retain their unsaved revision for retry.
#[derive(Debug)]
pub(super) struct DraftWriter {
    pending: watch::Sender<Option<Pending>>,
    completed: watch::Receiver<DraftStatus>,
    issued: AtomicU64,
}

impl DraftWriter {
    pub(super) fn start(
        history: SessionHistory,
        state: watch::Sender<SessionSnapshot>,
    ) -> Arc<Self> {
        let (pending, mut receiver) = watch::channel::<Option<Pending>>(None);
        let initial = state.borrow().draft_status();
        let (completed, completion) = watch::channel(initial.clone());
        tokio::spawn(async move {
            let mut saved_revision = initial.saved_revision;
            while receiver.changed().await.is_ok() {
                let Some(pending) = receiver.borrow_and_update().clone() else {
                    continue;
                };
                let revision = pending.draft.status.revision;
                let result = history.save_draft(pending.draft, saved_revision).await;
                let error = match result {
                    Ok(()) => {
                        saved_revision = revision;
                        None
                    }
                    Err(error) => Some(error),
                };
                state.send_modify(|snapshot| {
                    snapshot.draft_saved_revision = saved_revision;
                    snapshot.draft_save_attempt = pending.attempt;
                    snapshot.draft_save_error.clone_from(&error);
                    snapshot.sequence = snapshot.sequence.saturating_add(1);
                });
                completed.send_replace(DraftStatus {
                    revision,
                    saved_revision,
                    save_attempt: pending.attempt,
                    error,
                });
            }
        });
        Arc::new(Self {
            pending,
            completed: completion,
            issued: AtomicU64::new(0),
        })
    }

    pub(super) fn save(&self, draft: SavedDraft) {
        self.pending.send_if_modified(|pending| {
            if pending
                .as_ref()
                .is_some_and(|current| current.draft.status.revision > draft.status.revision)
            {
                return false;
            }
            let attempt = self.issued.fetch_add(1, Ordering::AcqRel).saturating_add(1);
            *pending = Some(Pending { attempt, draft });
            true
        });
    }

    pub(super) async fn flush(&self) -> DraftStatus {
        let issued = self.issued.load(Ordering::Acquire);
        let mut completed = self.completed.clone();
        loop {
            let current = completed.borrow_and_update().clone();
            if current.save_attempt >= issued {
                return current;
            }
            if completed.changed().await.is_err() {
                return DraftStatus {
                    error: Some(
                        "Your message could not be saved because the application closed.".into(),
                    ),
                    save_attempt: issued,
                    ..current
                };
            }
        }
    }
}

#[cfg(test)]
#[path = "session_drafts.tests.rs"]
mod tests;
