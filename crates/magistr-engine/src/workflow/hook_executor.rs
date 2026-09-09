//! Hook executor — dispatches resolved lifecycle hooks sequentially.
//!
//! The executor receives a list of `ResolvedHook`s and calls them one by one
//! through a `HookDispatcher` trait. Each hook's `on_failure` strategy
//! determines whether failures block execution (`FailFast`), get logged
//! and skipped (`Continue`), or produce warnings for prompt injection (`Warn`).

use super::lifecycle_hook::{HookAction, OnFailure, ResolvedHook};

/// Trait for dispatching a lifecycle hook action.
///
/// Implementations bridge to the actual MCP server or shell (or a mock in tests).
pub trait HookDispatcher: Send + Sync {
    /// Execute a hook action (MCP tool invocation or shell command).
    fn dispatch(
        &self,
        action: &HookAction,
    ) -> impl std::future::Future<Output = Result<HookOutput, HookError>> + Send;
}

/// Successful output from a hook execution.
#[derive(Debug, Clone)]
pub struct HookOutput {
    /// Optional message returned by the tool.
    pub message: Option<String>,
}

/// Error from a hook execution.
#[derive(Debug, Clone)]
pub struct HookError {
    /// Human-readable error description.
    pub message: String,
}

impl std::fmt::Display for HookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for HookError {}

/// Result of executing a single hook, tagged with its strategy.
#[derive(Debug, Clone)]
struct TaggedResult {
    name: String,
    on_failure: OnFailure,
    outcome: Result<HookOutput, HookError>,
}

/// Report from executing a batch of hooks.
#[derive(Debug, Clone)]
pub struct HookExecutionReport {
    /// Per-hook results in execution order.
    pub results: Vec<Result<HookOutput, HookError>>,
    /// Internal tagged results for warning extraction.
    tagged: Vec<TaggedResult>,
}

impl HookExecutionReport {
    /// Whether all hooks succeeded or only produced warnings (no hard failures).
    ///
    /// `Warn` failures are *not* considered hard failures.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.tagged
            .iter()
            .all(|r| r.outcome.is_ok() || r.on_failure == OnFailure::Warn)
    }

    /// Whether any hooks produced warnings (`Warn` strategy + failure).
    #[must_use]
    pub fn has_warnings(&self) -> bool {
        !self.warnings().is_empty()
    }

    /// Collect warning messages from hooks that failed with `Warn` strategy.
    #[must_use]
    pub fn warnings(&self) -> Vec<String> {
        self.tagged
            .iter()
            .filter(|r| r.on_failure == OnFailure::Warn && r.outcome.is_err())
            .map(|r| {
                let err = r.outcome.as_ref().unwrap_err();
                format!("Hook '{}' warning: {}", r.name, err.message)
            })
            .collect()
    }
}

/// Executes resolved lifecycle hooks sequentially through a dispatcher.
pub struct HookExecutor;

impl HookExecutor {
    /// Execute hooks in order, respecting each hook's `on_failure` strategy.
    ///
    /// - `FailFast` — stops execution immediately on failure
    /// - `Continue` — logs failure, continues to next hook
    /// - `Warn` — treats failure as warning, continues to next hook
    pub async fn execute(
        hooks: &[ResolvedHook],
        dispatcher: &(impl HookDispatcher + ?Sized),
    ) -> HookExecutionReport {
        let mut tagged = Vec::with_capacity(hooks.len());

        for hook in hooks {
            let outcome = dispatcher.dispatch(&hook.action).await;
            let is_fail_fast = hook.on_failure == OnFailure::FailFast && outcome.is_err();

            tagged.push(TaggedResult {
                name: hook.display_name().to_owned(),
                on_failure: hook.on_failure,
                outcome,
            });

            if is_fail_fast {
                break;
            }
        }

        let results = tagged.iter().map(|t| t.outcome.clone()).collect();
        HookExecutionReport { results, tagged }
    }
}

#[cfg(test)]
#[path = "hook_executor.tests.rs"]
mod tests;
