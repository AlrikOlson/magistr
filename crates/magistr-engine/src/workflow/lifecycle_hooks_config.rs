use serde::{Deserialize, Serialize};

use super::lifecycle_hook::LifecycleHook;

/// Container for lifecycle hooks with dedup/conflict validation.
///
/// Wraps a `Vec<LifecycleHook>` and provides validation on construction.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LifecycleHooksConfig {
    hooks: Vec<LifecycleHook>,
}

/// Validation error for lifecycle hooks configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HooksValidationError {
    /// Human-readable description of the conflict or issue.
    pub message: String,
}

impl std::fmt::Display for HooksValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "lifecycle hooks validation error: {}", self.message)
    }
}

impl std::error::Error for HooksValidationError {}

impl LifecycleHooksConfig {
    /// Create a new config from a list of hooks, validating for duplicates and security.
    ///
    /// Two hooks are considered duplicates if they have the same event and action display name.
    pub fn new(hooks: Vec<LifecycleHook>) -> Result<Self, HooksValidationError> {
        // Validate shell commands for security.
        for hook in &hooks {
            validate_hook_action(&hook.action)?;
        }
        // Check for duplicate (event, action name) pairs.
        for (i, a) in hooks.iter().enumerate() {
            for b in &hooks[i + 1..] {
                if a.event == b.event && a.display_name() == b.display_name() {
                    return Err(HooksValidationError {
                        message: format!(
                            "duplicate hook: event={}, action={}",
                            a.event,
                            a.display_name()
                        ),
                    });
                }
            }
        }
        Ok(Self { hooks })
    }

    /// Create an empty config (no hooks).
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Access the underlying hooks slice.
    #[must_use]
    pub fn hooks(&self) -> &[LifecycleHook] {
        &self.hooks
    }

    /// Whether this config has no hooks.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty()
    }

    /// Number of hooks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.hooks.len()
    }

    /// Add a hook, rejecting duplicates (same event + action name) and invalid shell commands.
    pub fn add(&mut self, hook: LifecycleHook) -> Result<(), HooksValidationError> {
        validate_hook_action(&hook.action)?;
        if self
            .hooks
            .iter()
            .any(|h| h.event == hook.event && h.display_name() == hook.display_name())
        {
            return Err(HooksValidationError {
                message: format!(
                    "duplicate hook: event={}, action={}",
                    hook.event,
                    hook.display_name()
                ),
            });
        }
        self.hooks.push(hook);
        Ok(())
    }

    /// Remove a hook by its index. Returns the removed hook, or an error if out of bounds.
    pub fn remove_by_index(&mut self, index: usize) -> Result<LifecycleHook, HooksValidationError> {
        if index >= self.hooks.len() {
            return Err(HooksValidationError {
                message: format!(
                    "index {} out of bounds (config has {} hooks)",
                    index,
                    self.hooks.len()
                ),
            });
        }
        Ok(self.hooks.remove(index))
    }

    /// Remove the first hook matching the given event and action name.
    ///
    /// The `name` parameter matches against the hook's display name (tool name
    /// for MCP hooks, command string for shell hooks).
    ///
    /// Returns the removed hook, or an error if no match is found.
    pub fn remove_by_event_and_name(
        &mut self,
        event: &super::lifecycle_event::LifecycleEvent,
        name: &str,
    ) -> Result<LifecycleHook, HooksValidationError> {
        let pos = self
            .hooks
            .iter()
            .position(|h| &h.event == event && h.display_name() == name);
        match pos {
            Some(i) => Ok(self.hooks.remove(i)),
            None => Err(HooksValidationError {
                message: format!("no hook found for event={event}, name={name}"),
            }),
        }
    }
}

// ── Security validation ────────────────────────────────────────────────

/// Protected directories that shell hooks must not target.
const PROTECTED_DIRS: &[&str] = &[
    "/etc",
    "/usr",
    "/bin",
    "/sbin",
    "/var",
    "/boot",
    "/sys",
    "/proc",
    "~/.ssh",
    "~/.gnupg",
    "~/.aws",
    "~/.config",
];

/// Dangerous command patterns to reject.
const DANGEROUS_PATTERNS: &[&str] = &[
    "rm -rf /",
    "rm -rf ~",
    "mkfs",
    "dd if=",
    ":(){",
    "chmod -R 777 /",
    "curl | sh",
    "curl | bash",
    "wget | sh",
    "wget | bash",
];

/// Validate a hook action for security concerns.
///
/// Only shell commands are validated — MCP tools are trusted (they go through
/// the MCP server's own authorization).
fn validate_hook_action(
    action: &super::lifecycle_hook::HookAction,
) -> Result<(), HooksValidationError> {
    if let super::lifecycle_hook::HookAction::ShellCommand { command, .. } = action {
        validate_shell_command(command)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "lifecycle_hooks_config.tests.rs"]
mod tests;

/// Validate a shell command against path traversal and dangerous patterns.
pub fn validate_shell_command(command: &str) -> Result<(), HooksValidationError> {
    // Reject path traversal.
    if command.contains("../") {
        return Err(HooksValidationError {
            message: format!("shell hook rejected: path traversal ('..') detected in: {command}"),
        });
    }

    // Reject commands targeting protected directories.
    for dir in PROTECTED_DIRS {
        if command.contains(dir) {
            return Err(HooksValidationError {
                message: format!(
                    "shell hook rejected: targets protected directory '{dir}' in: {command}"
                ),
            });
        }
    }

    // Reject dangerous patterns.
    for pattern in DANGEROUS_PATTERNS {
        if command.contains(pattern) {
            return Err(HooksValidationError {
                message: format!(
                    "shell hook rejected: dangerous pattern '{pattern}' in: {command}"
                ),
            });
        }
    }

    Ok(())
}
