//! Interactive initialization stays inside the resident owner's control lane.

use std::sync::{Arc, Mutex};

use agent_client_protocol::schema::v1::{SessionConfigOptionCategory, SessionConfigOptionValue};
use magistr_acp::{AcpEvent, AcpTransport};
use magistr_application::sessions::{AgentEvent, SessionOptions};

use super::{Inner, SessionResidency, acp_error};

pub(super) async fn initialize(
    transport: &AcpTransport,
    event: AcpEvent,
    options: SessionOptions,
    state: &Arc<Mutex<Inner>>,
) {
    let selected = if let Some(model) = options.model.as_deref() {
        let result = match &event {
            AcpEvent::Initialized {
                session_id,
                config_options,
                ..
            } => {
                if let Some(option) = config_options.as_deref().and_then(|options| {
                    options
                        .iter()
                        .find(|option| option.category == Some(SessionConfigOptionCategory::Model))
                }) {
                    transport
                        .set_config_option(
                            session_id.clone(),
                            option.id.to_string(),
                            SessionConfigOptionValue::ValueId {
                                value: model.to_owned().into(),
                            },
                        )
                        .await
                        .map(|accepted| {
                            let selected = crate::agent::translate::current_model(Some(&accepted));
                            let mut inner = state.lock().expect("supervisor state lock poisoned");
                            inner.config_options = accepted;
                            inner.history.observe_configuration(&inner.config_options);
                            selected
                        })
                        .map_err(|error| error.to_string())
                } else {
                    Err(format!(
                        "The agent does not advertise a model option for {model:?}"
                    ))
                }
            }
            _ => unreachable!("initialization runs only for the handshake event"),
        };
        match result {
            Ok(model) => model,
            Err(error) => {
                let mut inner = state.lock().expect("supervisor state lock poisoned");
                let error = format!("Failed to select session model: {error}");
                inner.finish(SessionResidency::Failed, &error);
                inner.emit(Err(acp_error(error)));
                return;
            }
        }
    } else {
        None
    };
    let mut inner = state.lock().expect("supervisor state lock poisoned");
    if inner.closing {
        return;
    }
    let mut translated = crate::agent::translate::translate(&Ok(event));
    if let (AgentEvent::Initialized { model, .. }, Some(selected)) = (&mut translated, selected) {
        *model = Some(selected);
    }
    inner.emit_cockpit(translated);
    if let Some(prompt) = options.opening_prompt.filter(|prompt| !prompt.is_empty())
        && inner.queue.front().is_some_and(|(id, _)| *id == 1)
    {
        inner.emit_cockpit(AgentEvent::UserText(format!("{}\n", prompt.text)));
    }
    inner.control_busy = false;
    inner.publish_activity();
}
