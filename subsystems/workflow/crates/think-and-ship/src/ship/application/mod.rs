//! Typed execution use cases shared by native clients and MCP adapters.

mod approval_gate_request;
mod check_admission;
mod check_outcome;
mod check_request;
mod check_settlement;
mod checks;
pub(crate) mod command_runner;
mod commands;
mod gate_opening;
mod gate_resolution;
mod gates;
mod objective_set;
mod plan_change;
mod plan_snapshot;
mod ship_error;
mod ship_report;
mod snapshot;
mod status;
mod task_admission;

pub use approval_gate_request::ApprovalGateRequest;
pub use check_admission::CheckAdmission;
pub use check_outcome::CheckOutcome;
pub use check_request::CheckRequest;
pub use check_settlement::CheckSettlement;
pub use gate_opening::GateOpening;
pub use gate_resolution::GateResolution;
pub use objective_set::ObjectiveSet;
pub use plan_change::PlanChange;
pub use plan_snapshot::PlanSnapshot;
pub use ship_error::ShipError;
pub use ship_report::ShipReport;
pub use snapshot::ShipSnapshot;
pub use status::ShipStatus;
pub use task_admission::TaskAdmission;

use std::sync::{Arc, Mutex, MutexGuard};

use super::engine::ShipEngine;

/// The application boundary over the same native engine served to every client.
#[derive(Clone)]
pub struct ShipUseCases {
    engine: Arc<Mutex<ShipEngine>>,
}

impl ShipUseCases {
    pub fn new(engine: Arc<Mutex<ShipEngine>>) -> Self {
        Self { engine }
    }

    fn lock(&self) -> Result<MutexGuard<'_, ShipEngine>, String> {
        let mut engine = self
            .engine
            .lock()
            .map_err(|_| "engine mutex poisoned".to_owned())?;
        engine.refresh_current()?;
        Ok(engine)
    }
}
