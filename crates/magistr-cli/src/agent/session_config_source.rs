//! Provenance of a frozen session configuration choice.

/// The authority that supplied a resolved value; contains no credentials or MCP environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionConfigSource {
    /// Supplied by this session's caller.
    Explicit,
    /// Inherited from the user's provider-scoped defaults.
    UserDefault,
    /// Chosen by runtime discovery.
    AutoDetected,
    /// Left to the provider, including restored conversation settings.
    RuntimeDefault,
    /// Preserved by a runtime-owned resumed conversation.
    RuntimeResume,
    /// Selected by project permission or model governance policy.
    ProjectPolicy,
    /// Loaded from the selected project's configuration file.
    ProjectFile,
}
