//! Compose the shared conversation owner with this host's admitted services.
//!
//! Provider configuration, planning policy and concrete project clients stay here;
//! queueing, context switching and relay lifetime belong to cockpit-runtime.

use crate::agent::AgentConfig;
use crate::application::workflow_client::EngineClient;
use magistr_application::sessions::{SessionHandle, SessionOptions, SessionPurpose};
use magistr_cockpit::checkpoint::CheckpointPort;
use magistr_cockpit_runtime::{InteractiveSession, channel::Inbox};

use super::ledger::{SpinePort, session_engine};
use super::loop_driver::Connector;

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
    let history_for = |purpose| {
        if config.history.purpose() == purpose {
            config.history.clone()
        } else {
            crate::agent::persistence::SessionHistory::for_purpose(purpose)
        }
    };
    let roadmap_history = history_for(SessionPurpose::Roadmap);
    let conversation_history = history_for(SessionPurpose::Conversation);
    let workspace = super::commands::workspace_port::workspace_port(&config.cwd);
    InteractiveSession::new(
        move |purpose, resume| {
            let mut config = match purpose {
                SessionPurpose::Roadmap => super::roadmap_request::config(&config),
                SessionPurpose::Conversation => {
                    let mut config = config.clone();
                    config.history = conversation_history.clone();
                    config.prompt.clear();
                    config.resume = None;
                    config.continue_conversation = false;
                    config
                }
            };
            if purpose == SessionPurpose::Roadmap {
                config.history = roadmap_history.clone();
            }
            // The live history keeps logical identity and current configuration;
            // the connector's resume identity belongs to this same provider.
            if resume.is_some() {
                config.model = config.history.current_model();
                config.effort = None;
            }
            config.resume = resume;
            let connector = connector.clone();
            async move {
                let (transport, spec) = connector
                    .connect(config)
                    .await
                    .map_err(|error| error.to_string())?;
                connector
                    .start_session(transport, spec, SessionOptions::new())
                    .await
                    .map(|handle| Box::new(handle) as Box<dyn SessionHandle>)
                    .map_err(|error| error.to_string())
            }
        },
        inbox,
        workspace,
        engines.as_ref().and_then(EngineClient::port),
        checkpoints,
    )
}

#[cfg(test)]
#[path = "interactive_session.tests.rs"]
mod tests;
