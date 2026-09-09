//! Plugin lifecycle management — init, activate, deactivate state machine.
//!
//! Wraps a [`PluginProcess`] with lifecycle state tracking. Enforces the
//! correct sequence: `spawn → init → activate → (use) → deactivate`.

use std::path::Path;

use super::process::PluginProcess;
use super::types::{DiscoveredPlugin, PluginError, PluginRequest, PluginResponse};

/// Lifecycle state of a plugin instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginState {
    /// Process spawned but not yet initialized.
    Spawned,
    /// Initialized — received init_ok with capabilities.
    Initialized,
    /// Activated for a specific project — ready for use.
    Active,
    /// Deactivated — no longer usable (process may still be alive).
    Deactivated,
}

impl std::fmt::Display for PluginState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawned => f.write_str("spawned"),
            Self::Initialized => f.write_str("initialized"),
            Self::Active => f.write_str("active"),
            Self::Deactivated => f.write_str("deactivated"),
        }
    }
}

/// A running plugin instance with lifecycle state tracking.
///
/// Manages the full lifecycle of an executable plugin:
/// `spawn → init → activate → deactivate`.
pub struct PluginInstance {
    /// The underlying subprocess.
    process: PluginProcess,
    /// Current lifecycle state.
    state: PluginState,
    /// Capabilities reported by the plugin during init.
    capabilities: Vec<String>,
    /// The discovered plugin metadata.
    plugin: DiscoveredPlugin,
}

impl PluginInstance {
    /// Spawn a new plugin instance from a discovered plugin.
    ///
    /// The plugin must have an entrypoint (executable plugin kinds only).
    pub async fn spawn(plugin: DiscoveredPlugin) -> Result<Self, PluginError> {
        let entrypoint = plugin.manifest.entrypoint.as_deref().ok_or_else(|| {
            PluginError::LifecycleViolation(format!(
                "plugin '{}' has no entrypoint — only executable plugins can be spawned",
                plugin.manifest.name
            ))
        })?;

        let process = PluginProcess::spawn(&plugin.manifest.name, &plugin.path, entrypoint).await?;

        Ok(Self {
            process,
            state: PluginState::Spawned,
            capabilities: Vec::new(),
            plugin,
        })
    }

    /// Initialize the plugin.
    ///
    /// Sends the `init` request with host metadata. Must be called before
    /// `activate()`. Transitions state from `Spawned` → `Initialized`.
    pub async fn init(&mut self, jig_version: &str) -> Result<&[String], PluginError> {
        if self.state != PluginState::Spawned {
            return Err(PluginError::LifecycleViolation(format!(
                "cannot init plugin '{}' in state '{}' (expected 'spawned')",
                self.plugin.manifest.name, self.state
            )));
        }

        let id = self.process.next_id();
        let request = PluginRequest::Init {
            id,
            jig_version: jig_version.to_string(),
            plugin_dir: self.plugin.path.display().to_string(),
        };

        let response = self.process.request(&request).await?;

        match response {
            PluginResponse::InitOk { capabilities, .. } => {
                self.capabilities = capabilities;
                self.state = PluginState::Initialized;
                Ok(&self.capabilities)
            }
            PluginResponse::Error { id, message } => Err(PluginError::PluginError { id, message }),
            other => Err(PluginError::ProcessIo(format!(
                "unexpected response to init from '{}': {other:?}",
                self.plugin.manifest.name
            ))),
        }
    }

    /// Activate the plugin for a specific project.
    ///
    /// Must be called after `init()`. Transitions state from
    /// `Initialized` → `Active`.
    pub async fn activate(&mut self, project_dir: &Path) -> Result<(), PluginError> {
        if self.state != PluginState::Initialized {
            return Err(PluginError::LifecycleViolation(format!(
                "cannot activate plugin '{}' in state '{}' (expected 'initialized')",
                self.plugin.manifest.name, self.state
            )));
        }

        let id = self.process.next_id();
        let request = PluginRequest::Activate {
            id,
            project_dir: project_dir.display().to_string(),
        };

        let response = self.process.request(&request).await?;

        match response {
            PluginResponse::ActivateOk { .. } => {
                self.state = PluginState::Active;
                Ok(())
            }
            PluginResponse::Error { id, message } => Err(PluginError::PluginError { id, message }),
            other => Err(PluginError::ProcessIo(format!(
                "unexpected response to activate from '{}': {other:?}",
                self.plugin.manifest.name
            ))),
        }
    }

    /// Deactivate the plugin (graceful shutdown).
    ///
    /// Can be called from `Initialized` or `Active` state.
    /// Transitions state to `Deactivated`.
    pub async fn deactivate(&mut self) -> Result<(), PluginError> {
        if self.state != PluginState::Active && self.state != PluginState::Initialized {
            return Err(PluginError::LifecycleViolation(format!(
                "cannot deactivate plugin '{}' in state '{}' (expected 'initialized' or 'active')",
                self.plugin.manifest.name, self.state
            )));
        }

        let id = self.process.next_id();
        let request = PluginRequest::Deactivate { id };

        let response = self.process.request(&request).await?;

        match response {
            PluginResponse::DeactivateOk { .. } => {
                self.state = PluginState::Deactivated;
                Ok(())
            }
            PluginResponse::Error { id, message } => Err(PluginError::PluginError { id, message }),
            other => Err(PluginError::ProcessIo(format!(
                "unexpected response to deactivate from '{}': {other:?}",
                self.plugin.manifest.name
            ))),
        }
    }

    /// Get the current lifecycle state.
    #[must_use]
    pub fn state(&self) -> PluginState {
        self.state
    }

    /// Get the capabilities reported during initialization.
    #[must_use]
    pub fn capabilities(&self) -> &[String] {
        &self.capabilities
    }

    /// Whether this plugin has a specific capability.
    #[must_use]
    pub fn has_capability(&self, cap: &str) -> bool {
        self.capabilities.iter().any(|c| c == cap)
    }

    /// Get the plugin manifest.
    #[must_use]
    pub fn manifest(&self) -> &super::types::PluginManifest {
        &self.plugin.manifest
    }

    /// Get direct access to the underlying process for protocol-level operations.
    ///
    /// Use this for backend-specific requests after the plugin is activated.
    pub fn process_mut(&mut self) -> &mut PluginProcess {
        &mut self.process
    }

    /// Get read-only access to the underlying process.
    #[must_use]
    pub fn process(&self) -> &PluginProcess {
        &self.process
    }

    /// Kill the plugin process immediately.
    pub fn kill(&mut self) {
        self.process.kill();
        self.state = PluginState::Deactivated;
    }
}

impl std::fmt::Debug for PluginInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginInstance")
            .field("state", &self.state)
            .field("capabilities", &self.capabilities)
            .field("plugin_name", &self.plugin.manifest.name)
            .finish_non_exhaustive()
    }
}

impl Drop for PluginInstance {
    fn drop(&mut self) {
        // PluginProcess::drop handles killing the child
    }
}

#[cfg(all(test, unix))]
#[path = "lifecycle.tests.rs"]
mod tests;
