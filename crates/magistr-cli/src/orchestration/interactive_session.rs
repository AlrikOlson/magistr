//! Compose the shared conversation owner with this host's admitted services.
//!
//! Provider configuration, planning policy and concrete project clients stay here;
//! queueing, context switching and relay lifetime belong to cockpit-runtime.

use crate::agent::{AgentConfig, cockpit_session::CockpitSession};
use crate::application::workflow_client::EngineClient;
use magistr_application::sessions::{SessionHandle, SessionOptions, SessionPolicy};
use magistr_cockpit::checkpoint::CheckpointPort;
use magistr_cockpit_runtime::{InteractiveSession, SessionPurpose, channel::Inbox};

use super::ledger::{SpinePort, session_engine};
use super::loop_driver::Connector;
use super::roadmap_request::RoadmapPolicy;

pub(crate) fn interactive_session(
    config: AgentConfig,
    connector: Connector,
    inbox: Inbox,
    engines: Option<EngineClient>,
) -> InteractiveSession {
    let checkpoints = engines.as_ref().map(|engines| {
        Box::new(SpinePort::at(
            session_engine(&config.cwd, engines.reader().clone()),
            &config.cwd,
        )) as Box<dyn CheckpointPort>
    });
    session_with_checkpoints(config, connector, inbox, engines, checkpoints)
}

pub(crate) fn session_with_checkpoints(
    config: AgentConfig,
    connector: Connector,
    inbox: Inbox,
    engines: Option<EngineClient>,
    checkpoints: Option<Box<dyn CheckpointPort>>,
) -> InteractiveSession {
    let roadmap_history = crate::agent::persistence::SessionHistory::new();
    let workbench = super::commands::workbench_port::workbench_port(&config.cwd);
    InteractiveSession::new(
        move |purpose, resume| {
            let mut config = match purpose {
                SessionPurpose::Roadmap => super::roadmap_request::config(&config),
                SessionPurpose::Conversation => {
                    let mut config = config.clone();
                    config.prompt.clear();
                    config.resume = None;
                    config.continue_conversation = false;
                    config
                }
            };
            // The coordinator owns one observed identity per purpose. Restore it
            // through the normal provider loader, which refuses unsupported loads.
            if resume.is_some() && purpose == SessionPurpose::Conversation {
                config.model = config.history.current_model();
                config.effort = None;
            }
            config.resume = resume;
            if purpose == SessionPurpose::Roadmap {
                config.history = roadmap_history.clone();
            }
            let connector = connector.clone();
            async move {
                let (transport, spec) = connector
                    .connect(config)
                    .await
                    .map_err(|error| error.to_string())?;
                let history = spec.history();
                let (policy, _) = spec.into_permission_policy();
                let policy: Box<dyn SessionPolicy> = match purpose {
                    SessionPurpose::Roadmap => Box::new(RoadmapPolicy::new(policy)),
                    SessionPurpose::Conversation => Box::new(policy),
                };
                CockpitSession::start_owned(transport, SessionOptions::new(), policy, history)
                    .map(|handle| Box::new(handle) as Box<dyn SessionHandle>)
                    .map_err(|error| error.to_string())
            }
        },
        inbox,
        workbench,
        engines.as_ref().and_then(EngineClient::port),
        checkpoints,
    )
}

#[cfg(test)]
#[path = "interactive_session.tests.rs"]
mod tests;
