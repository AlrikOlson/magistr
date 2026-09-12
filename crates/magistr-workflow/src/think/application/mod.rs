//! Synchronous reasoning use cases over the admitted native engine.
pub use crate::think::domain::{RecordedStep, StepError};

use crate::think::config::OutputFormat;
use crate::think::domain::ThinkStep;
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
            .map_err(|error| StepError {
                error,
                status: "failed",
                hint: "The admitted reasoning owner is unavailable.".to_owned(),
            })?
            .process_step_native(step)
    }

    pub fn pin_step(&self, step: u32, pinned: bool) -> Result<bool, String> {
        self.lock()?.pin_step(step, pinned)
    }

    pub fn get_step(&self, step: u32, latest: bool) -> Result<Option<ThinkStep>, String> {
        let engine = self.lock()?;
        Ok(if latest {
            engine.latest_revision_of(step)
        } else {
            engine.step_by_number(step)
        })
    }

    pub fn status(&self, verbose: bool) -> Result<serde_json::Value, String> {
        Ok(self.lock()?.status_snapshot(verbose))
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<serde_json::Value>, String> {
        Ok(self.lock()?.search_steps(query, limit.max(1)))
    }

    pub fn impact(&self, step: u32) -> Result<serde_json::Value, String> {
        self.lock()?.impact_of(step)
    }

    pub fn checkpoint(&self) -> Result<serde_json::Value, String> {
        Ok(self.lock()?.checkpoint_snapshot())
    }

    pub fn revise_estimate(&self, estimate: u32) -> Result<(u32, u32), String> {
        self.lock()?.revise_estimate(estimate)
    }

    pub fn set_branch_status(
        &self,
        id: &str,
        status: &str,
        merged_into: Option<u32>,
    ) -> Result<(&'static str, &'static str), String> {
        self.lock()?.set_branch_status(id, status, merged_into)
    }

    pub fn export(&self, format: &str) -> Result<String, String> {
        let engine = self.lock()?;
        match format.trim() {
            "" | "markdown" | "md" => Ok(engine.export_history(OutputFormat::Markdown)),
            "json" => Ok(engine.export_history(OutputFormat::Json)),
            "console" | "ansi" | "terminal" => Ok(engine.export_history(OutputFormat::Console)),
            "tree" | "branches" => Ok(engine.branch_tree()),
            other => Err(format!(
                "Unknown format \"{other}\". Use: markdown, json, console, tree."
            )),
        }
    }

    pub fn wipe(&self) -> Result<(), String> {
        self.lock()?.clear_history();
        Ok(())
    }
}
