//! Carrying one of the library's tool handlers out from synchronous code.

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use rmcp::ErrorData;
use rmcp::model::CallToolResult;
use serde_json::Value;

/// Run `handler` — a think-and-ship `#[tool]` method — to its answer and
/// read that answer the way a wire client would: the structured value on
/// success, the message of a soft `{ok:false, error_kind, message}`
/// refusal or of an MCP error as the `Err`.
///
/// The handlers behind every verb [`super::NativeEngines`] offers take an
/// engine lock, act and return; they are `async` because rmcp's router
/// requires the signature, not because they suspend. So they are polled
/// once with the standard no-op waker, which is exact for a future that
/// completes on its first poll. A handler that does suspend — a future
/// library version that awaits a process or a socket inside one of these —
/// is refused with a message rather than spun or blocked on: the verb
/// would then need the executor the transport has, and saying so is the
/// honest answer.
///
/// # Errors
/// The handler's own refusal, in its words.
pub(crate) fn settle<F>(handler: F) -> Result<Value, String>
where
    F: Future<Output = Result<CallToolResult, ErrorData>>,
{
    let mut handler = pin!(handler);
    let mut cx = Context::from_waker(Waker::noop());
    match handler.as_mut().poll(&mut cx) {
        Poll::Ready(Ok(result)) => answer(result),
        Poll::Ready(Err(error)) => Err(error.message.to_string()),
        Poll::Pending => Err(
            "the library's handler suspended instead of answering; this verb can only be \
             carried out where the handler's executor runs"
                .to_owned(),
        ),
    }
}

/// What a tool result says, read once: an MCP-level error is its text, a
/// soft refusal is its message, anything else is the structured value (or
/// the text content when the handler answered prose).
fn answer(result: CallToolResult) -> Result<Value, String> {
    let text = result
        .content
        .iter()
        .find_map(|block| block.as_text().map(|text| text.text.clone()));
    if result.is_error == Some(true) {
        return Err(text
            .or_else(|| result.structured_content.map(|value| value.to_string()))
            .unwrap_or_else(|| "the handler errored without a message".to_owned()));
    }
    let value = match result.structured_content {
        Some(value) => value,
        None => match text {
            Some(text) => serde_json::from_str(&text).unwrap_or(Value::String(text)),
            None => return Err("the handler answered nothing".to_owned()),
        },
    };
    if value.get("ok") == Some(&Value::Bool(false)) {
        let message = value["message"]
            .as_str()
            .filter(|message| !message.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| {
                format!(
                    "refused ({})",
                    value["error_kind"].as_str().unwrap_or("unknown")
                )
            });
        return Err(message);
    }
    Ok(value)
}

#[cfg(test)]
#[path = "handler_call.tests.rs"]
mod tests;
