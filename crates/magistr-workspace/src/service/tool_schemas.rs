use serde_json::Value;

/// The retained tool contracts, served verbatim: each tool's `inputSchema`
/// and the one `ResponseEnvelope` every tool returns as its `outputSchema`.
/// The files under `crates/magistr-workspace/schemas/tools` are
/// byte-identical to the reference tree's `schemas/` (an integration test
/// proves it), so nothing hand-writes a tool schema in Rust and a client
/// that validated against the reference validates against the native
/// server. Tools the reference never had keep their schemas under
/// `schemas/native`, outside that pin.
pub struct ToolSchemas;

impl ToolSchemas {
    /// The eight schema-defined tools; `workspace_status` is schema-less.
    pub const TOOL_NAMES: [&'static str; 8] = [
        "search", "read", "map", "edit", "files", "run", "script", "undo",
    ];
    /// Tools this server adds to the reference surface.
    pub const NATIVE_TOOL_NAMES: [&'static str; 1] = ["write"];
    pub const ENVELOPE: &'static str = include_str!("../../schemas/envelope.schema.json");

    /// The input schema text of one tool, or `None` for a name without one.
    #[must_use]
    pub fn input(tool: &str) -> Option<&'static str> {
        Some(match tool {
            "search" => include_str!("../../schemas/tools/search.input.schema.json"),
            "read" => include_str!("../../schemas/tools/read.input.schema.json"),
            "map" => include_str!("../../schemas/tools/map.input.schema.json"),
            "edit" => include_str!("../../schemas/tools/edit.input.schema.json"),
            "files" => include_str!("../../schemas/tools/files.input.schema.json"),
            "run" => include_str!("../../schemas/tools/run.input.schema.json"),
            "script" => include_str!("../../schemas/tools/script.input.schema.json"),
            "undo" => include_str!("../../schemas/tools/undo.input.schema.json"),
            "write" => include_str!("../../schemas/native/write.input.schema.json"),
            _ => return None,
        })
    }

    /// The parsed input schema of one tool.
    #[must_use]
    pub fn input_value(tool: &str) -> Option<Value> {
        serde_json::from_str(Self::input(tool)?).ok()
    }

    /// The parsed envelope schema.
    #[must_use]
    pub fn envelope_value() -> Value {
        serde_json::from_str(Self::ENVELOPE).unwrap_or(Value::Null)
    }
}
