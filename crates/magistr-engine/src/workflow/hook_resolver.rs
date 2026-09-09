use super::lifecycle_event::LifecycleEvent;
use super::lifecycle_hook::{LifecycleHook, ResolvedHook};

/// Resolves which hooks should fire for a given lifecycle event.
///
/// Pure function — no IO, no side effects. Filters the hook list by event
/// match and preserves declaration order.
pub struct HookResolver;

impl HookResolver {
    /// Filter and order hooks matching the given event.
    ///
    /// Returns hooks in declaration order (same order as the config).
    /// An empty config returns an empty vec.
    #[must_use]
    pub fn resolve(event: &LifecycleEvent, hooks: &[LifecycleHook]) -> Vec<ResolvedHook> {
        hooks
            .iter()
            .filter(|hook| &hook.event == event)
            .map(ResolvedHook::from)
            .collect()
    }
}

#[cfg(test)]
#[path = "hook_resolver.tests.rs"]
mod tests;
