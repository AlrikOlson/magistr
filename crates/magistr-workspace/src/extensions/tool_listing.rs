use serde::Serialize;

/// A listable plugin tool: derived wire name plus display metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PluginToolListing {
    pub name: String,
    pub title: String,
    pub description: String,
    pub plugin: String,
}
