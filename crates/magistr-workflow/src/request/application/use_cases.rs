use magistr_workflow_contracts::{Observed, WriteError};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::content::StructuredContent;
use crate::plan::engine::PlanEngine;
use crate::request::domain::{Request, RequestKind};
use crate::request::engine::RequestEngine;

use super::{RequestError, RequestInbox};

/// Request policy over the admitted native engines, beneath all transports.
#[derive(Clone)]
pub struct RequestUseCases {
    pub(super) engine: Arc<Mutex<RequestEngine>>,
    pub(super) plan: Option<Arc<Mutex<PlanEngine>>>,
    pub(super) promotion: Arc<Mutex<()>>,
}

impl RequestUseCases {
    pub fn new(engine: Arc<Mutex<RequestEngine>>) -> Self {
        Self {
            engine,
            plan: None,
            promotion: Arc::new(Mutex::new(())),
        }
    }

    pub fn with_plan(mut self, plan: Arc<Mutex<PlanEngine>>) -> Self {
        self.plan = Some(plan);
        self
    }

    pub fn engine(&self) -> Arc<Mutex<RequestEngine>> {
        Arc::clone(&self.engine)
    }

    /// The engine, current: every query and mutation observes the store
    /// before it answers or acts, so two roles on one store answer one
    /// state through a validated read of the selected store.
    pub(super) fn lock(&self) -> Result<MutexGuard<'_, RequestEngine>, RequestError> {
        let mut engine = self.engine.lock().map_err(|_| RequestError::Poisoned)?;
        engine
            .refresh_observed()
            .map_err(|error| RequestError::Store(WriteError::read(error)))?;
        Ok(engine)
    }

    pub fn capture(
        &self,
        kind: RequestKind,
        from: String,
        body: String,
        content: Option<StructuredContent>,
    ) -> Result<Observed<Request>, RequestError> {
        if body.trim().is_empty() {
            return Err(RequestError::InvalidArgs("request body is empty".into()));
        }
        if let Some(content) = &content {
            content.validate().map_err(RequestError::InvalidArgs)?;
        }
        self.lock()?
            .capture_with_content(kind, from, body, content)
            .map_err(RequestError::invalid_args)
    }

    pub fn get(&self, id: &str) -> Result<Request, RequestError> {
        self.lock()?
            .get(id)
            .cloned()
            .map_err(RequestError::NotFound)
    }

    pub fn status(&self) -> Result<RequestInbox, RequestError> {
        Ok(RequestInbox::from(self.lock()?.requests()))
    }

    pub fn link(&self, id: &str, cross_ref: &str) -> Result<Observed<Request>, RequestError> {
        self.lock()?
            .link(id, cross_ref)
            .map_err(RequestError::invalid_args)
    }

    pub fn research(
        &self,
        id: &str,
        summary: String,
        confidence: f64,
        sources: Vec<String>,
        note: Option<u32>,
    ) -> Result<Observed<Request>, RequestError> {
        if summary.trim().is_empty() {
            return Err(RequestError::InvalidArgs(
                "research summary is empty".into(),
            ));
        }
        self.lock()?
            .research(id, summary, confidence, sources, note)
            .map_err(RequestError::invalid_state)
    }

    pub fn ready(
        &self,
        min_confidence: Option<f64>,
        hints: &[String],
        limit: Option<u32>,
    ) -> Result<Vec<Request>, RequestError> {
        Ok(self
            .lock()?
            .ready(
                min_confidence.unwrap_or(0.6),
                hints,
                limit.unwrap_or(20) as usize,
            )
            .into_iter()
            .cloned()
            .collect())
    }

    pub fn raise(&self, id: &str) -> Result<Observed<Request>, RequestError> {
        self.lock()?.raise(id).map_err(RequestError::invalid_state)
    }

    pub fn snooze(
        &self,
        id: &str,
        minutes: Option<i64>,
    ) -> Result<Observed<Request>, RequestError> {
        self.lock()?
            .snooze(id, minutes.unwrap_or(60))
            .map_err(RequestError::invalid_state)
    }

    pub fn dismiss(&self, id: &str) -> Result<Observed<Request>, RequestError> {
        self.lock()?
            .dismiss(id)
            .map_err(RequestError::invalid_state)
    }
}

#[cfg(test)]
#[path = "use_cases.tests.rs"]
mod tests;
