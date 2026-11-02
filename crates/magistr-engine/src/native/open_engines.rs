use magistr_workflow::infra::{Broadcaster, Domain, Persistence};
use magistr_workflow::plan::PlanEngine;
use magistr_workflow::run::engine::RunEngine;
use magistr_workflow::run::persistence::{
    Persistence as RunPersistence, PersistenceConfig as RunPersistenceConfig,
};
use magistr_workflow::signal::SignalEngine;
use magistr_workflow::think::engine::ReasoningServer;

use super::native_config::NativeConfig;

/// Compose the four workflow owners from captured configuration and project identity.
///
/// Plan and signals use the project store, run uses the selected execution,
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
) -> (ReasoningServer, RunEngine, PlanEngine, SignalEngine) {
    let mut plan = open_plan(config, project_id.clone());
    let mut signal = open_signal(config, project_id.clone());
    let mut run = open_run(config, project_id.clone());
    let think_config = config.think_config(&project_id);
    let mut think = ReasoningServer::new_for_project(think_config, project_id);

    if let Some(b) = broadcaster {
        think = think.with_broadcaster(
            magistr_workflow::think::broadcast::Broadcaster::from_engine(b.clone()),
        );
        run = run.with_broadcaster(magistr_workflow::run::broadcast::Broadcaster::from_engine(
            b.clone().with_execution(config.execution_context().key()),
        ));
        plan = plan.with_broadcaster(magistr_workflow::plan::broadcast::Broadcaster::from_engine(
            b.clone(),
        ));
        signal = signal
            .with_broadcaster(magistr_workflow::signal::broadcast::Broadcaster::from_engine(b));
    }

    (think, run, plan, signal)
}

/// Open the project plan owner on its configured persistence.
pub(crate) fn open_plan(config: &NativeConfig, project_id: String) -> PlanEngine {
    PlanEngine::new(project_id)
        .with_persistence(Persistence::new(&config.persistence_config(), Domain::Plan))
}

/// Open the project signal owner on its configured persistence.
pub(crate) fn open_signal(config: &NativeConfig, project_id: String) -> SignalEngine {
    SignalEngine::new(project_id).with_persistence(Persistence::new(
        &config.persistence_config(),
        Domain::Signal,
    ))
}

/// Open the execution's plan and receipt owner with its authority binding.
pub(crate) fn open_run(config: &NativeConfig, project_id: String) -> RunEngine {
    let persist = config.persistence_config();
    let context = config.execution_context();
    RunEngine::new(project_id)
        .with_execution_binding(magistr_workflow::run::application::ExecutionBinding {
            working_copy: context.working_copy.clone(),
            execution_id: context.execution_id.clone(),
            owner_generation: config.owner_generation,
        })
        .with_persistence(
            RunPersistence::new(&RunPersistenceConfig {
                enabled: persist.enabled,
                data_dir: persist.data_dir,
            })
            .for_execution(&context.key()),
        )
}

#[cfg(test)]
#[path = "open_engines.tests.rs"]
mod tests;
