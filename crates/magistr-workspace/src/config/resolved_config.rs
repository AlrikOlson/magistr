use super::settings::Settings;
use super::{
    ConfigLayer, LayerReport, PolicyViolation, ResolveOptions, ResolvedKey, key_spec::KeySpec,
};
use crate::contracts::WorkspaceError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Effective workspace settings and deterministic diagnostic provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedConfig {
    pub config_version: u32,
    pub effective: BTreeMap<String, Value>,
    pub commands: BTreeMap<String, String>,
    pub trace: Vec<ResolvedKey>,
    pub warnings: Vec<String>,
    pub violations: Vec<PolicyViolation>,
    pub layers: Vec<LayerReport>,
}

impl Default for ResolvedConfig {
    fn default() -> Self {
        let trace: Vec<_> = KeySpec::REGISTRY
            .iter()
            .map(|(key, spec)| ResolvedKey {
                key: (*key).into(),
                value: spec.default_value(),
                decided_by: ConfigLayer::Defaults,
                pinned_by: None,
            })
            .collect();
        Self {
            config_version: 1,
            effective: trace
                .iter()
                .map(|key| (key.key.clone(), key.value.clone()))
                .collect(),
            commands: BTreeMap::new(),
            trace,
            warnings: vec![],
            violations: vec![],
            layers: ConfigLayer::ORDER
                .iter()
                .map(|&layer| LayerReport {
                    layer,
                    source: None,
                    present: layer == ConfigLayer::Defaults,
                    error: None,
                })
                .collect(),
        }
    }
}

impl ResolvedConfig {
    pub fn resolve(options: &ResolveOptions) -> Result<Self, WorkspaceError> {
        let mut config = Self::default();
        config.layers.truncate(1);
        let mut layers = Vec::new();
        for (layer, path) in options.layer_paths() {
            if let Some(settings) =
                Settings::load(layer, &path, &mut config.warnings, &mut config.layers)?
            {
                layers.push((layer, settings));
            }
        }
        let session = Settings::session(&options.session_flags);
        let present = !session.entries.is_empty() || session.commands.is_some();
        config.layers.push(LayerReport {
            layer: ConfigLayer::Session,
            source: None,
            present,
            error: None,
        });
        if present {
            layers.push((ConfigLayer::Session, session));
        }
        for (layer, settings) in &layers {
            config.merge_keys(*layer, settings);
        }
        config.merge_commands(&layers);
        config.effective = config
            .trace
            .iter()
            .filter(|key| !key.key.starts_with("commands."))
            .map(|key| (key.key.clone(), key.value.clone()))
            .collect();
        Ok(config)
    }

    fn source(&self, layer: ConfigLayer) -> &str {
        self.layers
            .iter()
            .find(|report| report.layer == layer)
            .and_then(|report| report.source.as_deref())
            .unwrap_or("flags")
    }

    fn merge_keys(&mut self, layer: ConfigLayer, settings: &Settings) {
        for (key, value) in &settings.entries {
            let Some((index, (_, spec))) = KeySpec::REGISTRY
                .iter()
                .enumerate()
                .find(|(_, (known, _))| known == key)
            else {
                self.warnings.push(format!(
                    "unknown config key '{key}' in {layer} config ({}) — ignored",
                    self.source(layer)
                ));
                continue;
            };
            if !spec.accepts(value) {
                self.warnings.push(format!(
                    "config key '{key}' in {layer} config ({}) must be {} — ignored",
                    self.source(layer),
                    spec.expected()
                ));
                continue;
            }
            let current = &mut self.trace[index];
            if matches!(spec, KeySpec::AdditiveList) {
                let mut merged = current.value.as_array().cloned().unwrap_or_default();
                for entry in value.as_array().into_iter().flatten() {
                    if !merged.contains(entry) {
                        merged.push(entry.clone());
                    }
                }
                current.value = Value::Array(merged);
            } else if current.pinned_by.is_some() && spec.loosens(&current.value, value) {
                self.violations.push(PolicyViolation {
                    code: "CONFIG_POLICY_DENIED".into(),
                    key: key.clone(),
                    attempted_by: layer,
                    attempted: value.clone(),
                    pinned_by: current.decided_by,
                    message: format!(
                        "{layer} config may not loosen '{key}' — {} pinned it to {} (SDD §7.2)",
                        current.decided_by, current.value
                    ),
                });
                continue;
            } else {
                current.value = value.clone();
            }
            current.decided_by = layer;
            current.pinned_by = spec.is_security().then_some(layer);
        }
    }

    fn merge_commands(&mut self, layers: &[(ConfigLayer, Settings)]) {
        let mut decided = BTreeMap::new();
        let mut pinned_by = None;
        for (layer, settings) in layers {
            let Some(commands) = &settings.commands else {
                continue;
            };
            for (name, raw) in commands {
                let Some(value) = raw.as_str() else {
                    self.warnings.push(format!(
                        "command '{name}' in {layer} config ({}) must be a string — ignored",
                        self.source(*layer)
                    ));
                    continue;
                };
                if let Some(pin) = pinned_by {
                    let pinned = self.commands.get(name);
                    if pinned.map(String::as_str) != Some(value) {
                        self.violations.push(PolicyViolation {
                            code: "CONFIG_POLICY_DENIED".into(), key: format!("commands.{name}"),
                            attempted_by: *layer, attempted: raw.clone(), pinned_by: pin,
                            message: format!("{layer} config may not {} command '{name}' — {pin} pins the run allowlist (SDD §7.2/§9.6)", if pinned.is_some() { "change" } else { "add" }),
                        });
                    }
                } else {
                    self.commands.insert(name.clone(), value.into());
                    decided.insert(name.clone(), *layer);
                }
            }
            if *layer == ConfigLayer::OrgPolicy {
                pinned_by = Some(*layer);
            }
        }
        for (name, value) in &self.commands {
            self.trace.push(ResolvedKey {
                key: format!("commands.{name}"),
                value: Value::String(value.clone()),
                decided_by: decided[name],
                pinned_by,
            });
        }
    }

    /// Diagnostics travel with future envelopes without changing policy outcome.
    #[must_use]
    pub fn notes(&self) -> Vec<String> {
        self.warnings
            .iter()
            .map(|warning| format!("config: {warning}"))
            .chain(
                self.violations.iter().map(|violation| {
                    format!("config: CONFIG_POLICY_DENIED — {}", violation.message)
                }),
            )
            .collect()
    }

    /// Pure doctor rendering; no lock, server, database, or configuration write.
    #[must_use]
    pub fn doctor(&self, root: &std::path::Path) -> String {
        let mut lines = vec![
            format!("workspace doctor config — workspace {}", root.display()),
            format!("config_version supported: {}", self.config_version),
            String::new(),
            "layer files (lowest precedence first):".into(),
        ];
        for layer in &self.layers {
            let source = layer
                .source
                .as_ref()
                .map(|source| format!("  {source}"))
                .unwrap_or_default();
            let status = if layer.error.is_some() {
                "unreadable"
            } else if layer.present {
                "read"
            } else {
                "absent"
            };
            lines.push(format!("  {:<10} {status}{source}", layer.layer.as_str()));
        }
        lines.extend([
            String::new(),
            "effective config (value ← deciding layer):".into(),
        ]);
        for entry in &self.trace {
            let value = entry
                .value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| entry.value.to_string());
            let pin = entry
                .pinned_by
                .map(|layer| format!(", pinned by {layer}"))
                .unwrap_or_default();
            lines.push(format!(
                "  {} = {value}  ({}{pin})",
                entry.key, entry.decided_by
            ));
        }
        if !self.warnings.is_empty() {
            lines.extend([String::new(), "warnings:".into()]);
            lines.extend(self.warnings.iter().map(|warning| format!("  {warning}")));
        }
        if !self.violations.is_empty() {
            lines.extend([
                String::new(),
                "policy denials (CONFIG_POLICY_DENIED):".into(),
            ]);
            lines.extend(
                self.violations
                    .iter()
                    .map(|violation| format!("  {}", violation.message)),
            );
        }
        lines.join("\n")
    }
}

#[cfg(test)]
#[path = "resolved_config.tests.rs"]
mod tests;
