//! Boundary-only response formatting, metadata decoding and deterministic secret redaction.
//! Native workspace use cases depend on contracts, never on this adapter.
mod redacted_text;
mod redactor;
mod response_adapter;
mod sealed_response;

pub use redacted_text::RedactedText;
pub use redactor::Redactor;
pub use response_adapter::ResponseAdapter;
pub use sealed_response::SealedResponse;
