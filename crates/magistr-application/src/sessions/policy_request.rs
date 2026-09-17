//! Data a session presents to its permission policy; response channels stay in the host.

use super::tool_verb::ToolVerb;
use alloc::string::String;

/// A complete, unbounded policy input. Observation truncation must never change
/// authorization scope. The host validates and decodes the JSON before evaluation;
/// malformed input must be denied, never treated as missing arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionPolicyRequest {
    /// The provider's tool-call identity, retained independently of display IDs.
    pub tool_call_id: String,
    /// The tool name or title used by the configured policy.
    pub tool_title: String,
    /// The complete compact JSON input, including URLs and every target path.
    pub input_json: String,
    /// The reported tool category; absence remains distinct from an unknown kind.
    pub tool_kind: Option<ToolVerb>,
}
