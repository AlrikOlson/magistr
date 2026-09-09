//! A single model discovered from provider probing.

/// A single model discovered from provider probing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeModel {
    /// Model identifier (e.g. `"claude-sonnet-4"`, `"gpt-4o"`).
    pub id: String,
    /// Human-readable display name (e.g. `"Sonnet 4"`, `"GPT-4o"`).
    pub name: String,
}
