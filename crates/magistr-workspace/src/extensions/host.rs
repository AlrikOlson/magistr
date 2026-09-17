use std::path::PathBuf;

use serde_json::{Value, json};

use super::{
    EffectiveCapabilities, ExtensionLane, HostClock, PluginComponent, PluginGuestPolicy,
    PluginHealth, PluginHealthRecord, PluginToolListing, ResolvedPlugin, discovery::Discovery,
};
use crate::{
    contracts::{EnvelopeVersion, ErrorKind, Freshness, WorkspaceError, WorkspaceResponse},
    query::budget,
    workspace_policy::WorkspacePolicy,
};

/// The plugin-tool host: inventory, manifests and grants are read per call,
/// the breaker persists beside them, and every guest runs through the lane.
/// Core tools never consult any of this — they continue at full fidelity by
/// construction, and no plugin can occupy a bare core name.
pub struct PluginHost<'a> {
    discovery: Discovery<'a>,
    health: PluginHealth,
    lane: &'a dyn ExtensionLane,
    clock: Box<dyn Fn() -> String + Send + Sync + 'a>,
}

impl<'a> PluginHost<'a> {
    /// `state_dir` is the admitted `.workbench` directory holding
    /// `plugins.json` and `plugin-health.json`.
    pub fn new(
        policy: &'a WorkspacePolicy,
        state_dir: PathBuf,
        lane: &'a dyn ExtensionLane,
    ) -> Self {
        Self {
            discovery: Discovery {
                policy,
                state_dir: state_dir.clone(),
            },
            health: PluginHealth::new(&state_dir),
            lane,
            clock: Box::new(HostClock::now),
        }
    }

    /// Inject the wall-clock used for breach records (tests pin it).
    #[must_use]
    pub fn with_clock(mut self, clock: impl Fn() -> String + Send + Sync + 'a) -> Self {
        self.clock = Box::new(clock);
        self
    }

    pub(super) fn discovery(&self) -> &Discovery<'a> {
        &self.discovery
    }

    pub(super) fn health(&self) -> &PluginHealth {
        &self.health
    }

    pub(super) fn lane(&self) -> &'a dyn ExtensionLane {
        self.lane
    }

    pub(super) fn now(&self) -> String {
        (self.clock)()
    }

    /// Every invocable plugin tool under its derived wire name.
    #[must_use]
    pub fn list_tools(&self) -> (Vec<PluginToolListing>, Vec<String>) {
        let (plugins, notes) = self.discovery.resolve_all();
        let tools = plugins
            .iter()
            .flat_map(|plugin| {
                plugin.manifest.tools.iter().map(|tool| PluginToolListing {
                    name: plugin.manifest.wire_tool_name(&tool.name),
                    title: format!("{} (plugin {})", tool.name, plugin.manifest.label()),
                    description: format!(
                        "{} [Plugin tool, sandboxed (SDD §8.1); source: {}{}. Output is host-wrapped in the standard envelope.]",
                        tool.description,
                        plugin.record.source.as_str(),
                        if plugin.record.source == super::PluginSource::Sideload {
                            ", untrusted"
                        } else {
                            ""
                        }
                    ),
                    plugin: plugin.record.name.clone(),
                })
            })
            .collect();
        (tools, notes)
    }

    /// True iff `name` addresses a plugin tool (contains the namespace slash).
    #[must_use]
    pub fn is_plugin_tool_name(name: &str) -> bool {
        name.contains('/')
    }

    /// Installed runner plugins (`runner-` packages) whose component is
    /// readable: the guests the run tool consults before its bundled
    /// parsers, named by their inventory entry. A plugin that cannot be
    /// resolved is a note, never a failure of the run tool.
    #[must_use]
    pub fn runner_components(&self) -> (Vec<(String, PluginComponent)>, Vec<String>) {
        let (plugins, mut notes) = self.discovery.resolve_all();
        let mut components = Vec::new();
        for plugin in plugins
            .iter()
            .filter(|plugin| plugin.record.pkg().starts_with("runner-"))
        {
            match self.component(plugin) {
                Ok(component) => components.push((plugin.record.name.clone(), component)),
                Err(error) => notes.push(format!("plugin {}: {error}", plugin.record.name)),
            }
        }
        (components, notes)
    }

    /// Invoke one plugin tool by wire name. `None` when no installed plugin
    /// derives that name — the caller answers its unknown-tool error.
    pub async fn run_tool(
        &self,
        wire_name: &str,
        args: &Value,
        budget: u64,
    ) -> Option<WorkspaceResponse<Value>> {
        let (plugins, mut notes) = self.discovery.resolve_all();
        let (plugin, tool) = plugins.iter().find_map(|plugin| {
            plugin
                .manifest
                .tools
                .iter()
                .find(|tool| plugin.manifest.wire_tool_name(&tool.name) == wire_name)
                .map(|tool| (plugin, tool.name.clone()))
        })?;
        let label = plugin.manifest.label();
        let component = match self.component(plugin) {
            Ok(component) => component,
            Err(error) => return Some(failure(error, notes)),
        };
        if let Some(breach) = self.health.breach(&label, &component.sha256) {
            return Some(failure(breakered(&label, &breach), notes));
        }
        let capabilities = EffectiveCapabilities::resolve(&plugin.record, &plugin.manifest);
        notes.extend(capabilities.notes);
        let payload = json!({"v": EnvelopeVersion::LATEST.get(), "tool": tool, "args": args});
        let outcome = self
            .lane
            .invoke_tool(
                component.bytes,
                plugin.manifest.invoke_export.clone(),
                payload.to_string(),
                capabilities.effective,
                PluginGuestPolicy::for_manifest(&plugin.manifest),
            )
            .await;
        let outcome = match outcome {
            Ok(outcome) => outcome,
            // Worker down or the sandbox lane absent — retryable, not a breach.
            Err(error) => {
                return Some(failure(
                    WorkspaceError::new(
                        ErrorKind::EngineUnavailable,
                        format!("plugin {label} could not run: {}", error.message),
                    ),
                    notes,
                ));
            }
        };
        if let crate::worker::WorkerGuestEnd::CapDenied(import) = &outcome.end {
            return Some(failure(
                WorkspaceError::new(
                    ErrorKind::PluginCapDenied,
                    format!(
                        "plugin {label} requires the import {import:?} which is not granted; widening a grant is a human install-time decision (SDD §8.4)"
                    ),
                ),
                notes,
            ));
        }
        if outcome.end != crate::worker::WorkerGuestEnd::Ok {
            let reason = outcome.reason();
            self.record_breach(&label, &component.sha256, &reason, &mut notes);
            return Some(failure(
                WorkspaceError::new(
                    ErrorKind::PluginCrashed {
                        plugin: label.clone(),
                    },
                    format!(
                        "plugin {label} {} ({reason}) and is now circuit-breakered unhealthy; core tools continue at full fidelity",
                        if outcome.kind() == "out-of-fuel" {
                            "exhausted its fuel"
                        } else {
                            "trapped"
                        }
                    ),
                ),
                notes,
            ));
        }
        let result = outcome.result.unwrap_or_default();
        let value = serde_json::from_str(&result).unwrap_or_else(|_| json!({ "raw": result }));
        notes.insert(
            0,
            format!(
                "plugin tool {wire_name} (source: {}); output host-wrapped (SDD §8.3)",
                plugin.record.source.as_str()
            ),
        );
        Some(budget::success(
            vec![value],
            budget,
            Freshness::Warm,
            notes,
            |_| String::new(),
            vec!["raise budget".into()],
        ))
    }

    fn component(&self, plugin: &ResolvedPlugin) -> Result<PluginComponent, WorkspaceError> {
        PluginComponent::resolve(
            self.discovery.policy,
            plugin.record.dir.as_deref().unwrap_or_default(),
            &plugin.manifest.component_path,
        )
        .ok_or_else(|| {
            WorkspaceError::new(
                ErrorKind::PluginCrashed {
                    plugin: plugin.manifest.label(),
                },
                format!(
                    "plugin {}: component {} is missing or outside the plugin directory; reinstall the plugin",
                    plugin.manifest.label(),
                    plugin.manifest.component_path
                ),
            )
        })
    }

    pub(super) fn record_breach(
        &self,
        plugin: &str,
        component_hash: &str,
        reason: &str,
        notes: &mut Vec<String>,
    ) {
        notes.extend(
            self.health
                .record(plugin, component_hash, reason, &self.now()),
        );
    }
}

fn breakered(label: &str, breach: &PluginHealthRecord) -> WorkspaceError {
    WorkspaceError::new(
        ErrorKind::PluginCrashed {
            plugin: label.to_owned(),
        },
        format!(
            "plugin {label} is circuit-breakered unhealthy since {} ({}); core tools continue at full fidelity. A changed component clears the breaker.",
            breach.at, breach.reason
        ),
    )
}

fn failure(error: WorkspaceError, notes: Vec<String>) -> WorkspaceResponse<Value> {
    WorkspaceResponse::Failure {
        version: EnvelopeVersion::LATEST,
        error,
        notes: (!notes.is_empty()).then_some(notes),
    }
}

#[cfg(test)]
#[path = "host.tests.rs"]
mod tests;
