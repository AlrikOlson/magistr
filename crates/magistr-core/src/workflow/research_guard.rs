//! Research guard — enforces that at least one research tool call (web_search,
//! firecrawl, web_fetch) occurred before allowing Research→Plan phase transition.
//!
//! Guard enforcement is now socket-based: the TUI process runs a guard socket
//! listener (`orchestration::workflow::workflow_socket`) and the MCP server queries it. This module
//! retains the `WorkflowGuard` trait impl for consistency with the guard system.

use crate::workflow::guards::WorkflowGuard;

/// Research guard that blocks Research→Plan transition without research evidence.
///
/// The actual enforcement is handled by `workflow_socket::SharedGuardState`
/// (in-process) and `workflow_socket::query_guard` (cross-process via socket).
/// This struct exists for the `WorkflowGuard` trait registry.
pub struct ResearchGuard;

impl WorkflowGuard for ResearchGuard {
    fn name(&self) -> &str {
        "research"
    }

    // Session-level hooks are no-ops — research validation happens at phase
    // transition time via the workflow guard socket.
}

#[cfg(test)]
#[path = "research_guard.tests.rs"]
mod tests;
