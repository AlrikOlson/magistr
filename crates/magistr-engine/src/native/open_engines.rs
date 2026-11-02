use magistr_workflow::decision::engine::ReasoningServer;
use magistr_workflow::infra::{Broadcaster, Domain, Persistence};
use magistr_workflow::plan::PlanEngine;
use magistr_workflow::request::RequestEngine;
use magistr_workflow::run::engine::RunEngine;
use magistr_workflow::run::persistence::{
    Persistence as RunPersistence, PersistenceConfig as RunPersistenceConfig,
};

use super::native_config::NativeConfig;

/// Compose the four workflow owners from captured configuration and project identity.
///
/// Plan and requests use the project store, run uses the selected execution,
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
) -> (ReasoningServer, RunEngine, PlanEngine, RequestEngine) {
    let mut plan = open_plan(config, project_id.clone());
    let mut request = open_request(config, project_id.clone());
    let mut run = open_run(config, project_id.clone());
    let decision_config = config.decision_config(&project_id);
    let mut decision = ReasoningServer::new_for_project(decision_config, project_id);

    if let Some(b) = broadcaster {
        decision = decision.with_broadcaster(
            magistr_workflow::decision::broadcast::Broadcaster::from_engine(b.clone()),
        );
        run = run.with_broadcaster(magistr_workflow::run::broadcast::Broadcaster::from_engine(
            b.clone().with_execution(config.execution_context().key()),
        ));
        plan = plan.with_broadcaster(magistr_workflow::plan::broadcast::Broadcaster::from_engine(
            b.clone(),
        ));
        request = request
            .with_broadcaster(magistr_workflow::request::broadcast::Broadcaster::from_engine(b));
    }

    (decision, run, plan, request)
}

/// Open the project plan owner on its configured persistence.
pub(crate) fn open_plan(config: &NativeConfig, project_id: String) -> PlanEngine {
    PlanEngine::new(project_id)
        .with_persistence(Persistence::new(&config.persistence_config(), Domain::Plan))
}

/// Open the project request owner on its configured persistence.
pub(crate) fn open_request(config: &NativeConfig, project_id: String) -> RequestEngine {
    RequestEngine::new(project_id).with_persistence(Persistence::new(
        &config.persistence_config(),
        Domain::Request,
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
