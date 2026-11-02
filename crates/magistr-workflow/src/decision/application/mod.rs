//! Synchronous reasoning use cases over the admitted native engine.
pub use crate::decision::domain::{NoteError, RecordedNote};
use magistr_workflow_contracts::{DecisionData, Observed};

use crate::decision::config::OutputFormat;
use crate::decision::domain::{Log, Note};
use crate::decision::engine::ReasoningServer;
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Clone)]
pub struct DecisionUseCases {
    engine: Arc<Mutex<ReasoningServer>>,
}

impl DecisionUseCases {
    pub fn new(engine: Arc<Mutex<ReasoningServer>>) -> Self {
        Self { engine }
    }

    pub fn engine(&self) -> Arc<Mutex<ReasoningServer>> {
        Arc::clone(&self.engine)
    }

    fn lock(&self) -> Result<MutexGuard<'_, ReasoningServer>, String> {
        self.engine
            .lock()
            .map_err(|_| "engine mutex poisoned".to_owned())
    }

    pub fn project_id(&self) -> Result<String, String> {
        Ok(self.lock()?.project_id().to_owned())
    }

    pub fn record(&self, note: Note) -> Result<RecordedNote, NoteError> {
        self.lock()
            .map_err(NoteError::from)?
            .process_note_native(note)
    }

    pub fn pin(&self, note: u32, pinned: bool) -> Result<Observed<bool>, NoteError> {
        self.lock().map_err(NoteError::from)?.pin(note, pinned)
    }

    pub fn get_note(&self, note: u32, latest: bool) -> Result<Option<Note>, String> {
        self.lock()?
            .observe_note(note, latest)
            .map(|read| read.value)
            .map_err(|error| error.to_string())
    }

    pub fn logs(&self) -> Result<Observed<DecisionData>, String> {
        self.lock()?
            .observe_logs()
            .map_err(|error| error.to_string())
    }

    pub fn log(&self, log: Option<&str>) -> Result<Observed<Option<Log>>, String> {
        self.lock()?
            .observe_log(log)
            .map_err(|error| error.to_string())
    }

    pub fn status(&self, verbose: bool, log: Option<&str>) -> Result<serde_json::Value, String> {
        let read = self
            .lock()?
            .analysis(log)
            .map_err(|error| error.to_string())?;
        let mut value = read.value.status_snapshot(verbose);
        value["source"] = serde_json::to_value(read.source).map_err(|error| error.to_string())?;
        Ok(value)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<serde_json::Value>, String> {
        Ok(self
            .lock()?
            .analysis(None)
            .map_err(|error| error.to_string())?
            .value
            .search_notes(query, limit.max(1)))
    }

    pub fn impact(&self, note: u32) -> Result<serde_json::Value, String> {
        self.lock()?
            .analysis_for_note(note)
            .map_err(|error| error.to_string())?
            .value
            .impact_of(note)
    }

    pub fn checkpoint(&self, log: Option<&str>) -> Result<serde_json::Value, String> {
        let read = self
            .lock()?
            .analysis(log)
            .map_err(|error| error.to_string())?;
        let mut value = read.value.checkpoint_snapshot();
        let status = read.value.status_snapshot(false);
        value["log_id"] = status["active_log"].clone();
        value["log_present"] = status["log_present"].clone();
        value["source"] = serde_json::to_value(read.source).map_err(|error| error.to_string())?;
        Ok(value)
    }

    pub fn revise_estimate(&self, estimate: u32) -> Result<Observed<(u32, u32)>, NoteError> {
        self.lock()
            .map_err(NoteError::from)?
            .revise_estimate(estimate)
    }

    pub fn set_thread_status(
        &self,
        id: &str,
        status: &str,
        merged_into: Option<u32>,
    ) -> Result<Observed<crate::decision::domain::ThreadStatusChange>, NoteError> {
        self.lock()
            .map_err(NoteError::from)?
            .set_thread_status(id, status, merged_into)
    }

    pub fn export(&self, format: &str, log: Option<&str>) -> Result<String, String> {
        let render = match format.trim() {
            "" | "markdown" | "md" => Some(OutputFormat::Markdown),
            "json" => Some(OutputFormat::Json),
            "console" | "ansi" | "terminal" => Some(OutputFormat::Console),
            "tree" | "threads" => None,
            other => {
                return Err(format!(
                    "Unknown format \"{other}\". Use: markdown, json, console, tree."
                ));
            }
        };
        let engine = self
            .lock()?
            .analysis(log)
            .map_err(|error| error.to_string())?
            .value;
        if !engine.status_snapshot(false)["log_present"]
            .as_bool()
            .unwrap_or(false)
        {
            return Err("No saved reasoning exists in this log.".into());
        }
        Ok(match render {
            Some(format) => engine.export_history(format),
            None => engine.thread_tree(),
        })
    }

    pub fn wipe(&self) -> Result<Observed<()>, NoteError> {
        self.lock().map_err(NoteError::from)?.clear_history()
    }
}
