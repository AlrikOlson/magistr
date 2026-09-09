use crate::types;

/// Parse a language string into its enum variant, falling back to `Unknown`.
pub fn parse_language_or_unknown(s: &str) -> types::Language {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .unwrap_or(types::Language::Unknown)
}
