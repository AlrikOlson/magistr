//! Surface-specific discovery, identity metadata and tool-error presentation.
use rmcp::model::ProtocolVersion;
use serde_json::{Map, Value};

pub(crate) trait SessionPolicy {
    fn initialize(&self, _requested: ProtocolVersion) -> Option<Value> {
        None
    }
    fn listing(&self) -> Option<Value> {
        None
    }
    fn session_meta(&self) -> Option<&'static str> {
        None
    }
    fn unavailable_call(
        &self,
        id: &Value,
        meta: Option<&Map<String, Value>>,
        reason: &str,
    ) -> Value;
}
