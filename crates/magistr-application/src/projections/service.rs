use magistr_domain::identity::RunId;

use crate::{ApplicationError, ProjectScope};

use super::{ProjectionPort, RunProjection};

/// Project-bound queries with exact response-scope correlation.
pub struct Projections<'a> {
    scope: &'a ProjectScope,
    port: &'a mut dyn ProjectionPort,
}

impl<'a> Projections<'a> {
    pub fn new(scope: &'a ProjectScope, port: &'a mut dyn ProjectionPort) -> Self {
        Self { scope, port }
    }

    pub async fn run(&mut self, run: &RunId) -> Result<RunProjection, ApplicationError> {
        self.scope.ensure_run(run)?;
        let projection = self.port.read_run(run).await?;
        if &projection.run != run {
            return Err(ApplicationError::InvalidResponse {
                reason: "projection belongs to another run".into(),
            });
        }
        if projection.event_sequence.is_some() && projection.owner_generation.is_none() {
            return Err(ApplicationError::InvalidResponse {
                reason: "an event sequence requires its owner generation".into(),
            });
        }
        Ok(projection)
    }
}

#[cfg(test)]
#[path = "service.tests.rs"]
mod tests;
