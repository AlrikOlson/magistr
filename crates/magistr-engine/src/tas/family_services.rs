//! The four think-and-ship family services, composed once.

use std::sync::{Arc, Mutex};

use think_and_ship::mcp::UnifiedService;
use think_and_ship::roadmap::application::RoadmapUseCases;
use think_and_ship::roadmap::{RoadmapEngine, RoadmapService};
use think_and_ship::ship::ShipService;
use think_and_ship::ship::application::ShipUseCases;
use think_and_ship::ship::engine::ShipEngine;
use think_and_ship::signal::application::SignalUseCases;
use think_and_ship::signal::{SignalEngine, SignalService};
use think_and_ship::think::ThinkService;
use think_and_ship::think::application::ThinkUseCases;
use think_and_ship::think::engine::ReasoningServer;

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
    /// reciprocal handles between roadmap and signal.
    pub(crate) fn compose(
        think: ReasoningServer,
        ship: ShipEngine,
        roadmap: RoadmapEngine,
        signal: SignalEngine,
    ) -> Self {
        let think = ThinkUseCases::new(Arc::new(Mutex::new(think)));
        let ship = ShipUseCases::new(Arc::new(Mutex::new(ship)));
        let roadmap = RoadmapUseCases::new(Arc::new(Mutex::new(roadmap)));
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
