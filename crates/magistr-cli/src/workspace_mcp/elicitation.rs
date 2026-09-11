//! MRTR (SDD §5.1, §9.4): present the aggregated dry-run diff to the client
//! mid-call and ask for the confirmation. True only on an explicit accept;
//! a decline, a cancel, a client failure or a malformed answer all keep the
//! token fallback — the CONFIRM_REQUIRED envelope the caller already holds.

use rmcp::{
    model::{
        ClientResult, ElicitRequest, ElicitRequestParams, ElicitationAction, ElicitationSchema,
        ServerRequest,
    },
    service::{Peer, RoleServer},
};
use serde_json::Value;

/// Ceiling on the diff text an elicitation message carries.
pub(crate) const ELICIT_DIFF_CHARS: usize = 4000;

/// The message is built from the SEALED envelope — elicitation is a response
/// surface, and redaction has exactly one choke point.
pub(crate) fn message(sealed: &Value) -> String {
    let error = sealed["error"]["message"].as_str().unwrap_or_default();
    let notes: String = sealed["notes"]
        .as_array()
        .map(|notes| {
            notes
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
        .chars()
        .take(ELICIT_DIFF_CHARS)
        .collect();
    format!("{error}\n\n{notes}\n\nApply these changes?")
}

pub(crate) async fn confirm(peer: &Peer<RoleServer>, sealed: &Value) -> bool {
    let Ok(requested_schema) = ElicitationSchema::builder()
        .required_bool_with("apply", |apply| {
            apply
                .title("Apply")
                .description("Apply the diff above to the workspace")
        })
        .build()
    else {
        return false;
    };
    let request = ServerRequest::ElicitRequest(ElicitRequest::new(
        ElicitRequestParams::FormElicitationParams {
            meta: None,
            message: message(sealed),
            requested_schema,
        },
    ));
    match peer.send_request(request).await {
        Ok(ClientResult::ElicitResult(result)) => {
            result.action == ElicitationAction::Accept
                && result
                    .content
                    .as_ref()
                    .and_then(|content| content.get("apply"))
                    .and_then(Value::as_bool)
                    == Some(true)
        }
        // An MRTR-capable client that still fails the elicitation gets the
        // token fallback: the envelope it receives carries the token.
        _ => false,
    }
}

#[cfg(test)]
#[path = "elicitation.tests.rs"]
mod tests;
