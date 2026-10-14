use magistr_workflow_contracts::{Observed, WriteError};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::content::StructuredContent;
use crate::roadmap::engine::RoadmapEngine;
use crate::signal::domain::{Signal, SignalKind};
use crate::signal::engine::SignalEngine;

use super::{SignalError, SignalInbox};

/// Signal policy over the admitted native engines, beneath all transports.
#[derive(Clone)]
pub struct SignalUseCases {
    pub(super) engine: Arc<Mutex<SignalEngine>>,
    pub(super) roadmap: Option<Arc<Mutex<RoadmapEngine>>>,
    pub(super) promotion: Arc<Mutex<()>>,
}

impl SignalUseCases {
    pub fn new(engine: Arc<Mutex<SignalEngine>>) -> Self {
        Self {
            engine,
            roadmap: None,
            promotion: Arc::new(Mutex::new(())),
        }
    }

    pub fn with_roadmap(mut self, roadmap: Arc<Mutex<RoadmapEngine>>) -> Self {
        self.roadmap = Some(roadmap);
        self
    }

    pub fn engine(&self) -> Arc<Mutex<SignalEngine>> {
        Arc::clone(&self.engine)
    }

    /// The engine, current: every query and mutation observes the store
    /// before it answers or acts, so two roles on one store answer one
    /// state through a validated read of the selected store.
    pub(super) fn lock(&self) -> Result<MutexGuard<'_, SignalEngine>, SignalError> {
        let mut engine = self.engine.lock().map_err(|_| SignalError::Poisoned)?;
        engine
            .refresh_observed()
            .map_err(|error| SignalError::Store(WriteError::read(error)))?;
        Ok(engine)
    }

    pub fn capture(
        &self,
        kind: SignalKind,
        from: String,
        body: String,
        content: Option<StructuredContent>,
    ) -> Result<Observed<Signal>, SignalError> {
        if body.trim().is_empty() {
            return Err(SignalError::InvalidArgs("signal body is empty".into()));
        }
        if let Some(content) = &content {
            content.validate().map_err(SignalError::InvalidArgs)?;
        }
        self.lock()?
            .capture_with_content(kind, from, body, content)
            .map_err(SignalError::invalid_args)
    }

    pub fn get(&self, id: &str) -> Result<Signal, SignalError> {
        self.lock()?.get(id).cloned().map_err(SignalError::NotFound)
    }

    pub fn status(&self) -> Result<SignalInbox, SignalError> {
        Ok(SignalInbox::from(self.lock()?.signals()))
    }

    pub fn link(&self, id: &str, cross_ref: &str) -> Result<Observed<Signal>, SignalError> {
        self.lock()?
            .link(id, cross_ref)
            .map_err(SignalError::invalid_args)
    }

    pub fn research(
        &self,
        id: &str,
        summary: String,
        confidence: f64,
        sources: Vec<String>,
        think_step: Option<u32>,
    ) -> Result<Observed<Signal>, SignalError> {
        if summary.trim().is_empty() {
            return Err(SignalError::InvalidArgs("research summary is empty".into()));
        }
        self.lock()?
            .research(id, summary, confidence, sources, think_step)
            .map_err(SignalError::invalid_state)
    }

    pub fn pending(
        &self,
        min_confidence: Option<f64>,
        hints: &[String],
        limit: Option<u32>,
    ) -> Result<Vec<Signal>, SignalError> {
        Ok(self
            .lock()?
            .pending(
                min_confidence.unwrap_or(0.6),
                hints,
                limit.unwrap_or(20) as usize,
            )
            .into_iter()
            .cloned()
            .collect())
    }

    pub fn surface(&self, id: &str) -> Result<Observed<Signal>, SignalError> {
        self.lock()?.surface(id).map_err(SignalError::invalid_state)
    }

    pub fn snooze(&self, id: &str, minutes: Option<i64>) -> Result<Observed<Signal>, SignalError> {
        self.lock()?
            .snooze(id, minutes.unwrap_or(60))
            .map_err(SignalError::invalid_state)
    }

    pub fn ignore(&self, id: &str) -> Result<Observed<Signal>, SignalError> {
        self.lock()?.ignore(id).map_err(SignalError::invalid_state)
    }
}

#[cfg(test)]
#[path = "use_cases.tests.rs"]
mod tests;
