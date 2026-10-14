//! One logical conversation's history facts and settlement writer change together.
//! Cloned configuration carries this context across provider reconnections; a
//! separate conversation gets a fresh context. Only its session owner saves it.

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use agent_client_protocol::schema::v1::{ContentBlock, SessionUpdate, ToolCallContent};
use chrono::Utc;
use magistr_acp::AcpEvent;

use super::{session_outcome::SessionOutcome, session_record::SessionRecord, storage};
use crate::{
    config::inputs::ProjectInputs,
    error::{MagistrError, Result},
};

const SUMMARY_CHARS: usize = 2048;
const FILE_LIMIT: usize = 512;
static ISSUED: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub(crate) struct SessionHistory(Arc<Mutex<State>>);

struct State {
    id: String,
    record: Option<SessionRecord>,
    path: Option<PathBuf>,
    started: Option<Instant>,
    active_turn: bool,
    summary: String,
    reported_cost_usd: Option<f64>,
    inputs: Option<ProjectInputs>,
}

impl SessionHistory {
    pub(crate) fn new() -> Self {
        Self(Arc::new(Mutex::new(State {
            id: format!(
                "{}-{}-{}",
                std::process::id(),
                Utc::now().timestamp_nanos_opt().unwrap_or_default(),
                ISSUED.fetch_add(1, Ordering::Relaxed)
            ),
            record: None,
            path: None,
            started: None,
            active_turn: false,
            summary: String::new(),
            reported_cost_usd: None,
            inputs: None,
        })))
    }

    pub(crate) fn from_record(record: SessionRecord) -> Self {
        let history = Self::new();
        {
            let mut state = history.0.lock().expect("history state lock poisoned");
            state.id = record.session_id.clone();
            state.reported_cost_usd = record.cost_usd;
            state.record = Some(record);
        }
        history
    }

    pub(crate) async fn begin(
        &self,
        inputs: &ProjectInputs,
        cwd: &Path,
        provider: &str,
        model: Option<&str>,
    ) -> Result<()> {
        let captured_inputs = inputs.clone();
        let inputs = inputs.clone();
        let root = cwd.to_path_buf();
        let (project, workspace, owner, path) = tokio::task::spawn_blocking(move || {
            let location = magistr_engine::native::ProjectLocation::resolve(&root)
                .map_err(|error| MagistrError::io(&root, error))?;
            Ok::<_, MagistrError>((
                location.authority_root.display().to_string(),
                location.working_copy.display().to_string(),
                crate::team::Identity::resolve(&root, &inputs)
                    .id()
                    .to_owned(),
                storage::sessions_db_path(&inputs),
            ))
        })
        .await
        .map_err(|error| {
            MagistrError::Other(format!("Could not prepare session history: {error}"))
        })??;
        let mut state = self.0.lock().expect("history state lock poisoned");
        if let Some(record) = &state.record
            && (record.workspace != workspace || record.provider != provider)
        {
            return Err(MagistrError::Config(
                "The saved conversation belongs to another project or agent.".into(),
            ));
        }
        if state.record.is_none() {
            state.record = Some(SessionRecord {
                session_id: state.id.clone(),
                name: None,
                project,
                workspace,
                provider: provider.to_owned(),
                provider_session_id: None,
                resume_supported: false,
                started_at: Utc::now(),
                updated_at: Utc::now(),
                ended_at: None,
                outcome: None,
                cost_usd: None,
                turns: Some(0),
                duration_ms: 0,
                files_changed: Vec::new(),
                tags: Vec::new(),
                summary: None,
                model: model.map(str::to_owned),
                owner: Some(owner),
            });
        }
        if let Some(record) = &mut state.record {
            record.ended_at = None;
            record.outcome = None;
        }
        state.path = path;
        state.inputs = Some(captured_inputs);
        state.started = Some(Instant::now());
        state.active_turn = false;
        state.summary.clear();
        Ok(())
    }

    pub(crate) fn conversation(&self) -> Option<magistr_application::sessions::Conversation> {
        let state = self.0.lock().expect("history state lock poisoned");
        state
            .record
            .as_ref()
            .map(|record| magistr_application::sessions::Conversation {
                id: record.session_id.clone(),
                name: record.name.clone(),
                provider: record.provider.clone(),
            })
    }

    pub(crate) fn current_model(&self) -> Option<String> {
        self.0
            .lock()
            .expect("history state lock poisoned")
            .record
            .as_ref()
            .and_then(|record| record.model.clone())
    }

    pub(crate) fn rename(&self, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 120 || name.chars().any(char::is_control) {
            return Err(MagistrError::Config(
                "Use a conversation name with 1 to 120 characters and no line breaks.".into(),
            ));
        }
        let mut state = self.0.lock().expect("history state lock poisoned");
        let record = state.record.as_mut().ok_or_else(|| {
            MagistrError::Config("Wait for the conversation to open before naming it.".into())
        })?;
        record.name = Some(name.to_owned());
        Ok(())
    }

    pub(crate) fn observe_configuration(
        &self,
        options: &[agent_client_protocol::schema::v1::SessionConfigOption],
    ) {
        let mut state = self.0.lock().expect("history state lock poisoned");
        if let Some(record) = &mut state.record {
            record.model = crate::agent::translate::current_model(Some(options));
        }
    }

    pub(crate) fn bind_transport(&self, transport: &magistr_acp::AcpTransport) {
        let mut state = self.0.lock().expect("history state lock poisoned");
        if let Some(record) = &mut state.record {
            record.provider_session_id = transport
                .session_id()
                .filter(|id| !id.is_empty())
                .map(str::to_owned);
            record.resume_supported = transport
                .agent_capabilities()
                .is_some_and(|caps| caps.load_session);
            if let Some(model) =
                crate::agent::translate::current_model(transport.session_config_options())
            {
                record.model = Some(model);
            }
        }
    }

    pub(crate) fn begin_turn(&self) {
        let mut state = self.0.lock().expect("history state lock poisoned");
        state.active_turn = true;
        state.summary.clear();
    }

    pub(crate) fn finished_turn(&self) {
        let mut state = self.0.lock().expect("history state lock poisoned");
        state.active_turn = false;
        keep_summary(&mut state);
        if let Some(record) = &mut state.record {
            record.turns = record.turns.map(|turns| turns.saturating_add(1));
        }
    }

    /// The latest reported cumulative USD amount, including a reported zero.
    pub(crate) fn reported_cost_usd(&self) -> Option<f64> {
        self.0
            .lock()
            .expect("history state lock poisoned")
            .reported_cost_usd
    }

    pub(crate) fn observe(&self, event: &AcpEvent) {
        let mut state = self.0.lock().expect("history state lock poisoned");
        if let AcpEvent::SessionUpdate { update, .. } = event
            && let SessionUpdate::UsageUpdate(usage) = &**update
            && let Some(cost) = usage
                .cost
                .as_ref()
                .and_then(crate::agent::translate::reported_usd)
        {
            state.reported_cost_usd = Some(cost);
        }
        if state.record.is_none() {
            return;
        }
        match event {
            AcpEvent::Initialized {
                session_id,
                agent_capabilities,
                config_options,
                ..
            } => {
                let record = state.record.as_mut().expect("record checked");
                record.provider_session_id = (!session_id.is_empty()).then(|| session_id.clone());
                record.resume_supported = agent_capabilities.load_session;
                if let Some(model) =
                    crate::agent::translate::current_model(config_options.as_deref())
                {
                    record.model = Some(model);
                }
            }
            AcpEvent::SessionUpdate { update, .. } => match &**update {
                SessionUpdate::AgentMessageChunk(chunk) if state.active_turn => {
                    if let ContentBlock::Text(text) = &chunk.content {
                        let remaining = SUMMARY_CHARS.saturating_sub(state.summary.chars().count());
                        state.summary.extend(text.text.chars().take(remaining));
                    }
                }
                SessionUpdate::ConfigOptionUpdate(update) => {
                    if let Some(model) =
                        crate::agent::translate::current_model(Some(&update.config_options))
                    {
                        state.record.as_mut().expect("record checked").model = Some(model);
                    }
                }
                SessionUpdate::ToolCall(call) => files(&mut state, &call.content),
                SessionUpdate::ToolCallUpdate(update) => {
                    if let Some(content) = &update.fields.content {
                        files(&mut state, content);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn settlement(
        &self,
        outcome: SessionOutcome,
    ) -> Option<(Option<PathBuf>, SessionRecord, Option<ProjectInputs>)> {
        let mut state = self.0.lock().expect("history state lock poisoned");
        let started = state.started.take()?;
        let path = state.path.clone();
        let inputs = state.inputs.clone();
        let cost = state.reported_cost_usd;
        keep_summary(&mut state);
        let record = state.record.as_mut()?;
        record.updated_at = Utc::now();
        record.ended_at = Some(record.updated_at);
        record.cost_usd = cost;
        record.duration_ms = record
            .duration_ms
            .saturating_add(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX));
        record.outcome = Some(outcome);
        Some((path, record.clone(), inputs))
    }

    /// Restore only this conversation's draft, without creating storage.
    pub(crate) async fn load_draft(
        &self,
    ) -> std::result::Result<crate::application::SavedDraft, String> {
        let (path, id) = {
            let state = self.0.lock().expect("history state lock poisoned");
            (state.path.clone(), state.id.clone())
        };
        tokio::task::spawn_blocking(move || {
            let path =
                path.ok_or_else(|| "No folder is available for saved conversations.".to_owned())?;
            if !path.exists() {
                return Ok(crate::application::SavedDraft::default());
            }
            let conn = storage::open_readonly_db(&path).map_err(super::draft::read_failure)?;
            super::draft::load(&conn, &id)
        })
        .await
        .map_err(super::draft::read_failure)?
    }

    /// The draft worker serializes checkpoints for this history owner. Its
    /// expected durable revision prevents another writer's edit being replaced.
    pub(crate) async fn save_draft(
        &self,
        draft: crate::application::SavedDraft,
        expected: u64,
    ) -> std::result::Result<(), String> {
        let checkpoint = {
            let state = self.0.lock().expect("history state lock poisoned");
            let mut record = state.record.clone().ok_or_else(|| {
                "Wait for the conversation to open before saving a message.".to_owned()
            })?;
            record.updated_at = Utc::now();
            record.cost_usd = state.reported_cost_usd;
            if let Some(started) = state.started {
                record.duration_ms = record.duration_ms.saturating_add(
                    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                );
            }
            (state.path.clone(), record)
        };
        tokio::task::spawn_blocking(move || {
            let path = checkpoint
                .0
                .ok_or_else(|| "No folder is available for saved conversations.".to_owned())?;
            let conn = storage::open_db(&path).map_err(super::draft::save_failure)?;
            super::draft::write(&conn, &checkpoint.1, &draft, expected)
        })
        .await
        .map_err(super::draft::save_failure)?
    }

    pub(crate) async fn settle(&self, outcome: SessionOutcome) -> Result<()> {
        let Some((path, record, inputs)) = self.settlement(outcome) else {
            return Ok(());
        };
        tokio::task::spawn_blocking(move || save(path, &record, inputs.as_ref()))
            .await
            .map_err(|error| {
                MagistrError::Other(format!("Could not save session history: {error}"))
            })?
    }

    /// Forced task cancellation still schedules the same writer. Normal
    /// termination awaits it; callers keep the runtime alive through cleanup.
    pub(crate) fn settle_detached(&self, outcome: SessionOutcome) {
        let Some((path, record, inputs)) = self.settlement(outcome) else {
            return;
        };
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn_blocking(move || {
                if let Err(error) = save(path, &record, inputs.as_ref()) {
                    warn(&error);
                }
            });
        } else {
            tracing::warn!(
                "Session history could not be saved because the application runtime ended."
            );
        }
    }
}

fn save(
    path: Option<PathBuf>,
    record: &SessionRecord,
    inputs: Option<&ProjectInputs>,
) -> Result<()> {
    if let Some(inputs) = inputs {
        let mut spend = crate::cost::SessionSpend::new(&record.session_id, record.cost_usd)
            .with_run(record.turns.unwrap_or_default(), record.duration_ms);
        if let Some(model) = &record.model {
            spend = spend.with_model(model);
        }
        crate::cost::record_session(inputs, Path::new(&record.workspace), &spend);
    }
    let path = path.ok_or_else(|| {
        MagistrError::Config("No directory is available for saved sessions.".into())
    })?;
    let conn = storage::open_db(&path)?;
    storage::insert_session(&conn, record)
}

pub(crate) fn warn(error: &MagistrError) {
    tracing::warn!(%error, "The session ended, but its history could not be saved.");
}

fn keep_summary(state: &mut State) {
    let summary = std::mem::take(&mut state.summary);
    if !summary.is_empty()
        && let Some(record) = &mut state.record
    {
        record.summary = Some(summary);
    }
}

fn files(state: &mut State, content: &[ToolCallContent]) {
    let record = state.record.as_mut().expect("record checked by observer");
    for item in content {
        if let ToolCallContent::Diff(diff) = item {
            let path = diff.path.display().to_string();
            if record.files_changed.len() < FILE_LIMIT && !record.files_changed.contains(&path) {
                record.files_changed.push(path);
            }
        }
    }
}

#[cfg(test)]
#[path = "history.tests.rs"]
mod tests;
