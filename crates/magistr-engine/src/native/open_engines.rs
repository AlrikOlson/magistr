use magistr_workflow::infra::{Broadcaster, Domain, Persistence};
use magistr_workflow::roadmap::RoadmapEngine;
use magistr_workflow::ship::engine::ShipEngine;
use magistr_workflow::ship::persistence::{
    Persistence as ShipPersistence, PersistenceConfig as ShipPersistenceConfig,
};
use magistr_workflow::signal::SignalEngine;
use magistr_workflow::think::engine::ReasoningServer;

use super::native_config::NativeConfig;

/// Compose the four workflow owners from captured configuration and project identity.
///
/// Roadmap and signals use the project store, ship uses the selected execution,
/// and reasoning uses its configured session. An admitted writer supplies the
/// shared broadcaster; an observer opens these owners without a publisher.
///
/// Construction loads available data without repairing or rewriting it.
/// Current queries validate the selected source through each owner's observation API.
#[must_use]
pub(crate) fn open_engines(
    config: &NativeConfig,
    project_id: String,
    broadcaster: Option<Broadcaster>,
) -> (ReasoningServer, ShipEngine, RoadmapEngine, SignalEngine) {
    let mut roadmap = open_roadmap(config, project_id.clone());
    let mut signal = open_signal(config, project_id.clone());
    let mut ship = open_ship(config, project_id.clone());
    let think_config = config.think_config(&project_id);
    let mut think = ReasoningServer::new_for_project(think_config, project_id);

    if let Some(b) = broadcaster {
        think = think.with_broadcaster(
            magistr_workflow::think::broadcast::Broadcaster::from_engine(b.clone()),
        );
        ship = ship.with_broadcaster(magistr_workflow::ship::broadcast::Broadcaster::from_engine(
            b.clone().with_execution(config.execution_context().key()),
        ));
        roadmap = roadmap.with_broadcaster(
            magistr_workflow::roadmap::broadcast::Broadcaster::from_engine(b.clone()),
        );
        signal = signal
            .with_broadcaster(magistr_workflow::signal::broadcast::Broadcaster::from_engine(b));
    }

    (think, ship, roadmap, signal)
}

/// Open the project roadmap owner on its configured persistence.
pub(crate) fn open_roadmap(config: &NativeConfig, project_id: String) -> RoadmapEngine {
    RoadmapEngine::new(project_id).with_persistence(Persistence::new(
        &config.persistence_config(),
        Domain::Roadmap,
    ))
}

/// Open the project signal owner on its configured persistence.
pub(crate) fn open_signal(config: &NativeConfig, project_id: String) -> SignalEngine {
    SignalEngine::new(project_id).with_persistence(Persistence::new(
        &config.persistence_config(),
        Domain::Signal,
    ))
}

/// Open the execution's plan and receipt owner with its authority binding.
pub(crate) fn open_ship(config: &NativeConfig, project_id: String) -> ShipEngine {
    let persist = config.persistence_config();
    let context = config.execution_context();
    ShipEngine::new(project_id)
        .with_execution_binding(magistr_workflow::ship::application::ExecutionBinding {
            working_copy: context.working_copy.clone(),
            execution_id: context.execution_id.clone(),
            owner_generation: config.owner_generation,
        })
        .with_persistence(
            ShipPersistence::new(&ShipPersistenceConfig {
                enabled: persist.enabled,
                data_dir: persist.data_dir,
            })
            .for_execution(&context.key()),
        )
}

#[cfg(test)]
#[path = "open_engines.tests.rs"]
mod tests;
