use super::guard_context::GuardContext;
use super::guard_result::GuardResult;

/// Lifecycle guard that fires at session/commit boundaries.
///
/// Guards never block the agent. They return [`GuardAction::SteerPrompt`] to
/// inject guidance or [`GuardAction::Warn`] to emit a non-blocking warning.
///
/// Concrete implementations live in `magistr-cli` (they require IO).
/// The engine defines the trait so that guard results can flow through
/// the protocol layer without coupling to filesystem details.
pub trait WorkflowGuard: Send + Sync {
    /// Human-readable name for this guard (e.g., "plan", "spec", "clean_tree").
    fn name(&self) -> &str;

    /// Fires before an agent session starts.
    fn pre_session(&self, _ctx: &GuardContext) -> GuardResult {
        GuardResult::ok(self.name())
    }

    /// Fires after an agent session completes.
    fn post_session(&self, _ctx: &GuardContext) -> GuardResult {
        GuardResult::ok(self.name())
    }

    /// Fires before a commit is created.
    fn pre_commit(&self, _ctx: &GuardContext) -> GuardResult {
        GuardResult::ok(self.name())
    }

    /// Fires after a commit is created.
    fn post_commit(&self, _ctx: &GuardContext) -> GuardResult {
        GuardResult::ok(self.name())
    }
}
