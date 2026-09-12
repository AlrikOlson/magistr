//! The four think-and-ship family services, composed once.

use std::sync::{Arc, Mutex};

use magistr_workflow::mcp::UnifiedService;
use magistr_workflow::roadmap::application::RoadmapUseCases;
use magistr_workflow::roadmap::{RoadmapEngine, RoadmapService};
use magistr_workflow::ship::ShipService;
use magistr_workflow::ship::application::EvidencePort;
use magistr_workflow::ship::application::ShipUseCases;
use magistr_workflow::ship::engine::ShipEngine;
use magistr_workflow::signal::application::SignalUseCases;
use magistr_workflow::signal::{SignalEngine, SignalService};
use magistr_workflow::think::ThinkService;
use magistr_workflow::think::application::ThinkUseCases;
use magistr_workflow::think::engine::ReasoningServer;

/// An engine as its service holds it: shared, locked per call.
type Shared<T> = Arc<Mutex<T>>;

/// Native use cases composed once under project admission. Transports are
/// disposable views over these capabilities, with no separate workflow state.
#[derive(Clone)]
pub(crate) struct FamilyServices {
    pub(crate) think: ThinkUseCases,
    pub(crate) ship: ShipUseCases,
    pub(crate) roadmap: RoadmapUseCases,
    pub(crate) signal: SignalUseCases,
}

impl FamilyServices {
    /// Wrap freshly opened engines in their services and wire the
    /// reciprocal handles: roadmap ↔ signal, and — when the host composed
    /// an evidence capability — ship as the roadmap's completion authority,
    /// so a chunk completes only on the run's current receipts whichever
    /// adapter asks.
    pub(crate) fn compose(
        think: ReasoningServer,
        ship: ShipEngine,
        roadmap: RoadmapEngine,
        signal: SignalEngine,
        evidence: Option<Arc<dyn EvidencePort>>,
    ) -> Self {
        let think = ThinkUseCases::new(Arc::new(Mutex::new(think)));
        let mut ship = ShipUseCases::new(Arc::new(Mutex::new(ship)));
        let mut roadmap = RoadmapUseCases::new(Arc::new(Mutex::new(roadmap)));
        if let Some(evidence) = evidence {
            ship = ship.with_evidence(evidence);
            roadmap = roadmap.with_completion_evidence(ship.clone());
        }
        let signal =
            SignalUseCases::new(Arc::new(Mutex::new(signal))).with_roadmap(roadmap.engine());
        let roadmap = roadmap.with_signal(signal.engine());
        Self {
            think,
            ship,
            roadmap,
            signal,
        }
    }

    /// The MCP transport's view of the same services.
    pub(crate) fn unified(&self) -> UnifiedService {
        UnifiedService::new(
            ThinkService::from_use_cases(self.think.clone()),
            ShipService::from_use_cases(self.ship.clone()),
            RoadmapService::from_use_cases(self.roadmap.clone()),
            SignalService::from_use_cases(self.signal.clone()),
        )
    }

    /// The shared engine handles the services act on — what the reader
    /// queries, so reads and native mutations share the same state.
    pub(crate) fn engines(
        &self,
    ) -> (
        Shared<ReasoningServer>,
        Shared<ShipEngine>,
        Shared<RoadmapEngine>,
        Shared<SignalEngine>,
    ) {
        (
            self.think.engine(),
            self.ship.engine(),
            self.roadmap.engine(),
            self.signal.engine(),
        )
    }
}
