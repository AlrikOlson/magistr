//! The four think-and-ship family services, composed once.

use std::sync::{Arc, Mutex};

use think_and_ship::mcp::UnifiedService;
use think_and_ship::roadmap::{RoadmapEngine, RoadmapService};
use think_and_ship::ship::ShipService;
use think_and_ship::ship::engine::ShipEngine;
use think_and_ship::signal::{SignalEngine, SignalService};
use think_and_ship::think::ThinkService;
use think_and_ship::think::engine::ReasoningServer;

/// An engine as its service holds it: shared, locked per call.
type Shared<T> = Arc<Mutex<T>>;

/// The library's own tool handlers over the four engines — the one
/// implementation of every native mutation. Composed exactly as the
/// standalone `think-and-ship` binary composes them (the reciprocal
/// wire-layer handles included: `signal_promote` creates chunks,
/// `roadmap_status` folds in the pending-signal count), whichever surface
/// is asking: the MCP transport serves them as its [`UnifiedService`], and
/// the in-process writer ([`super::NativeEngines`]) calls the same handler
/// methods directly. There is no second implementation to drift from.
#[derive(Clone)]
pub(crate) struct FamilyServices {
    pub(crate) think: ThinkService,
    pub(crate) ship: ShipService,
    pub(crate) roadmap: RoadmapService,
    pub(crate) signal: SignalService,
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
        let think = ThinkService::new(think);
        let ship = ShipService::new(ship);
        let roadmap = RoadmapService::new(roadmap);
        let signal = SignalService::new(signal).with_roadmap(roadmap.engine());
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
            self.think.clone(),
            self.ship.clone(),
            self.roadmap.clone(),
            self.signal.clone(),
        )
    }

    /// The shared engine handles the services act on — what the reader
    /// queries, so a query and a handler never look at different engines.
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
