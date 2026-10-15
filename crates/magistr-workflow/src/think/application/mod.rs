//! Synchronous reasoning use cases over the admitted native engine.
pub use crate::think::domain::{RecordedStep, StepError};
use magistr_workflow_contracts::{Observed, ThinkData};

use crate::think::config::OutputFormat;
use crate::think::domain::{ThinkHistory, ThinkStep};
use crate::think::engine::ReasoningServer;
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Clone)]
pub struct ThinkUseCases {
    engine: Arc<Mutex<ReasoningServer>>,
}

impl ThinkUseCases {
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

    pub fn record_step(&self, step: ThinkStep) -> Result<RecordedStep, StepError> {
        self.lock()
            .map_err(StepError::from)?
            .process_step_native(step)
    }

    pub fn pin_step(&self, step: u32, pinned: bool) -> Result<Observed<bool>, StepError> {
        self.lock().map_err(StepError::from)?.pin_step(step, pinned)
    }

    pub fn get_step(&self, step: u32, latest: bool) -> Result<Option<ThinkStep>, String> {
        self.lock()?
            .observe_step(step, latest)
            .map(|read| read.value)
            .map_err(|error| error.to_string())
    }

    pub fn sessions(&self) -> Result<Observed<ThinkData>, String> {
        self.lock()?
            .observe_sessions()
            .map_err(|error| error.to_string())
    }

    pub fn session(&self, session: Option<&str>) -> Result<Observed<Option<ThinkHistory>>, String> {
        self.lock()?
            .observe_session(session)
            .map_err(|error| error.to_string())
    }

    pub fn status(
        &self,
        verbose: bool,
        session: Option<&str>,
    ) -> Result<serde_json::Value, String> {
        let read = self
            .lock()?
            .analysis(session)
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
            .search_steps(query, limit.max(1)))
    }

    pub fn impact(&self, step: u32) -> Result<serde_json::Value, String> {
        self.lock()?
            .analysis_for_step(step)
            .map_err(|error| error.to_string())?
            .value
            .impact_of(step)
    }

    pub fn checkpoint(&self, session: Option<&str>) -> Result<serde_json::Value, String> {
        let read = self
            .lock()?
            .analysis(session)
            .map_err(|error| error.to_string())?;
        let mut value = read.value.checkpoint_snapshot();
        let status = read.value.status_snapshot(false);
        value["session_id"] = status["active_session"].clone();
        value["session_present"] = status["session_present"].clone();
        value["source"] = serde_json::to_value(read.source).map_err(|error| error.to_string())?;
        Ok(value)
    }

    pub fn revise_estimate(&self, estimate: u32) -> Result<Observed<(u32, u32)>, StepError> {
        self.lock()
            .map_err(StepError::from)?
            .revise_estimate(estimate)
    }

    pub fn set_branch_status(
        &self,
        id: &str,
        status: &str,
        merged_into: Option<u32>,
    ) -> Result<Observed<crate::think::domain::BranchStatusChange>, StepError> {
        self.lock()
            .map_err(StepError::from)?
            .set_branch_status(id, status, merged_into)
    }

    pub fn export(&self, format: &str, session: Option<&str>) -> Result<String, String> {
        let render = match format.trim() {
            "" | "markdown" | "md" => Some(OutputFormat::Markdown),
            "json" => Some(OutputFormat::Json),
            "console" | "ansi" | "terminal" => Some(OutputFormat::Console),
            "tree" | "branches" => None,
            other => {
                return Err(format!(
                    "Unknown format \"{other}\". Use: markdown, json, console, tree."
                ));
            }
        };
        let engine = self
            .lock()?
            .analysis(session)
            .map_err(|error| error.to_string())?
            .value;
        if !engine.status_snapshot(false)["session_present"]
            .as_bool()
            .unwrap_or(false)
        {
            return Err("No saved reasoning exists in this session.".into());
        }
        Ok(match render {
            Some(format) => engine.export_history(format),
            None => engine.branch_tree(),
        })
    }

    pub fn wipe(&self) -> Result<Observed<()>, StepError> {
        self.lock().map_err(StepError::from)?.clear_history()
    }
}
