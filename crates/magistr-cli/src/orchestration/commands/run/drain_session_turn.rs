use super::*;
use crate::agent;
use color_eyre::eyre;

/// Drain a single turn from an agent session, collecting text output.
///
/// Returns `Ok(output)` with the collected assistant text from this turn.
/// Returns `Err` if the session produced an error result.
///
/// This is the testable core of the run command — it accepts a pre-built
/// `AgentSession` (which can use a channel-based mock in tests).
pub async fn drain_session_turn(
    session: &mut agent::session::AgentSession,
) -> eyre::Result<DrainResult> {
    let mut collected_text = String::new();

    while let Some(msg_result) = session.next_message().await {
        match msg_result {
            Ok(msg) => match &msg {
                crate::agent::message::AgentMessage::Assistant { message, .. } => {
                    for block in &message.content {
                        if let crate::agent::message::ContentBlock::Text { text } = block {
                            collected_text.push_str(text);
                        }
                    }
                }
                crate::agent::message::AgentMessage::Result {
                    is_error,
                    num_turns,
                    duration_ms,
                    total_cost_usd,
                    session_id,
                    ..
                } => {
                    return Ok(DrainResult {
                        text: collected_text,
                        is_error: *is_error,
                        num_turns: *num_turns,
                        duration_ms: *duration_ms,
                        total_cost_usd: *total_cost_usd,
                        session_id: session_id.as_str().to_string(),
                    });
                }
                _ => {}
            },
            Err(e) => {
                return Err(eyre::eyre!("Agent stream error: {e}"));
            }
        }
    }

    // Stream ended without a Result message
    Ok(DrainResult {
        text: collected_text,
        is_error: false,
        num_turns: 0,
        duration_ms: 0,
        total_cost_usd: None,
        session_id: String::new(),
    })
}
