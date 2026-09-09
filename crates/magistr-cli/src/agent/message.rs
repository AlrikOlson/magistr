//! Backend-agnostic message types for agent sessions.
//!
//! Canonical definitions now live in `magistr-core`. This module re-exports
//! everything for backward compatibility so existing `crate::agent::message::*`
//! imports continue to work.

// Re-export all message types from magistr-core.
pub use magistr_core::{
    AgentMessage, AssistantContent, ContentBlock, SessionId, UserBody, UserContent,
};

/// Category of tool being invoked (mirrors ACP `ToolKind`).
///
/// Re-exported from `magistr_core::ToolKind` — the canonical definition
/// lives in the core crate so both TUI and GUI frontends share it.
pub type ToolUseKind = magistr_core::ToolKind;

// Re-exported from magistr-core. Canonical definitions live there.
pub use magistr_core::ToolLocation;

// ─── ACP Plan types ─────────────────────────────────────────────────────────

// Canonical definitions live in magistr-core. Re-export for backward compatibility.
pub use magistr_core::{
    AcpConfigOption, AcpPlanEntry, AgentCommand, ConfigOptionCategory, ConfigSelectValue,
    PlanEntryPriority, PlanEntryStatus,
};

#[cfg(test)]
#[path = "message.tests.rs"]
mod tests;
