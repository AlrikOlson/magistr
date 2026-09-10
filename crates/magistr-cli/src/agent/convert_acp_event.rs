//! Convert wire events without taking session lifecycle ownership.

use magistr_acp::AcpEvent;
use tracing::debug;

use super::message::{AgentMessage, SessionId};
use super::types;

/// Convert a typed `AcpEvent` into `AgentMessage` values.
///
/// Returns a tuple of (messages, is_result, is_cancelled) so the reader task
/// can track session lifecycle state.
pub(super) fn convert_acp_event(event: &AcpEvent) -> (Vec<AgentMessage>, bool, bool) {
    let event_type = match event {
        AcpEvent::SessionUpdate { .. } => "session_update",
        AcpEvent::PromptResponse { .. } => "prompt_response",
        AcpEvent::Initialized { .. } => "initialized",
        AcpEvent::Terminal { .. } => "terminal",
        AcpEvent::System { .. } => "system",
    };
    debug!(event_type, "converting ACP event");
    match event {
        AcpEvent::SessionUpdate { update, session_id } => {
            let sid = SessionId::new(session_id);
            let messages = types::messages_from_session_update(update, &sid);
            (messages, false, false)
        }
        AcpEvent::PromptResponse {
            response,
            session_id,
        } => {
            let sid = SessionId::new(session_id);
            let msg = types::message_from_prompt_response(response, &sid, 0, 1, None);
            let is_result = matches!(&msg, AgentMessage::Result { .. });
            let is_cancelled = matches!(&msg, AgentMessage::Cancelled { .. });
            (vec![msg], is_result, is_cancelled)
        }
        AcpEvent::Initialized {
            agent_capabilities,
            session_id,
            config_options,
            ..
        } => {
            // Emit a system message so the TUI can display initialization info
            let sys_msg = AgentMessage::System {
                subtype: "initialized".to_string(),
                data: serde_json::json!({
                    "session_id": session_id,
                    "load_session": agent_capabilities.load_session,
                    "prompt_capabilities": {
                        "image": agent_capabilities.prompt_capabilities.image,
                        "audio": agent_capabilities.prompt_capabilities.audio,
                        "embedded_context": agent_capabilities.prompt_capabilities.embedded_context,
                    },
                    "session_list": agent_capabilities.session_capabilities.list.is_some(),
                }),
            };
            let mut msgs = vec![sys_msg];

            // If config options were returned from session/new or session/load,
            // emit a ConfigUpdate message so the TUI picks them up (C7.6)
            if let Some(opts) = config_options
                && !opts.is_empty()
            {
                let options = types::config_options_from_acp(opts);
                msgs.push(AgentMessage::ConfigUpdate { options });
            }

            (msgs, false, false)
        }
        // A terminal's raw bytes ride the wire whole, as a byte array, so
        // a remote cockpit's pane paints what a local one does.
        AcpEvent::Terminal { terminal_id, bytes } => {
            let msg = AgentMessage::System {
                subtype: "terminal_output".to_string(),
                data: serde_json::json!({ "terminal_id": terminal_id, "bytes": bytes }),
            };
            (vec![msg], false, false)
        }
        AcpEvent::System { subtype, data } => {
            let msg = AgentMessage::System {
                subtype: subtype.clone(),
                data: data.clone(),
            };
            (vec![msg], false, false)
        }
    }
}
