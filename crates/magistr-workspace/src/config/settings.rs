use super::{ConfigLayer, LayerReport};
use crate::contracts::{ErrorKind, WorkspaceError};
use serde_json::Value;
use std::path::Path;

/// Parsed settings preserve document order independently of the effective map.
#[derive(Default)]
pub(super) struct Settings {
    pub(super) entries: Vec<(String, Value)>,
    pub(super) commands: Option<Vec<(String, Value)>>,
}

impl Settings {
    pub(super) fn load(
        layer: ConfigLayer,
        path: &Path,
        warnings: &mut Vec<String>,
        reports: &mut Vec<LayerReport>,
    ) -> Result<Option<Self>, WorkspaceError> {
        let source = path.to_string_lossy().into_owned();
        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                reports.push(LayerReport {
                    layer,
                    source: Some(source),
                    present: false,
                    error: None,
                });
                return Ok(None);
            }
            Err(error) => {
                Self::unreadable(layer, source, error.to_string(), warnings, reports);
                return Ok(None);
            }
        };
        let doc = match content.parse::<toml::Table>() {
            Ok(doc) => doc,
            Err(error) => {
                Self::unreadable(layer, source, error.to_string(), warnings, reports);
                return Ok(None);
            }
        };
        reports.push(LayerReport {
            layer,
            source: Some(source.clone()),
            present: true,
            error: None,
        });
        Self::flatten(doc, layer, &source, warnings).map(Some)
    }

    fn unreadable(
        layer: ConfigLayer,
        source: String,
        reason: String,
        warnings: &mut Vec<String>,
        reports: &mut Vec<LayerReport>,
    ) {
        warnings.push(format!(
            "{layer} config ({source}) is not valid TOML — layer skipped: {reason}"
        ));
        reports.push(LayerReport {
            layer,
            source: Some(source),
            present: false,
            error: Some(reason),
        });
    }

    fn flatten(
        doc: toml::Table,
        layer: ConfigLayer,
        source: &str,
        warnings: &mut Vec<String>,
    ) -> Result<Self, WorkspaceError> {
        let mut settings = Self::default();
        for (key, value) in doc {
            match key.as_str() {
                "config_version" => {
                    let version = value.as_integer().map(|value| value as f64).or_else(|| {
                        value
                            .as_float()
                            .filter(|value| value.is_finite() && value.fract() == 0.0)
                    });
                    if let Some(version) = version {
                        if version > 1.0 {
                            return Err(WorkspaceError::new(
                                ErrorKind::UnsupportedVersion,
                                format!(
                                    "config layer {layer} ({source}) declares config_version {version}; this binary supports 1"
                                ),
                            ));
                        }
                    } else {
                        warnings.push(format!("config_version in {layer} config ({source}) must be an integer — ignored"));
                    }
                }
                "commands" => {
                    if let toml::Value::Table(commands) = value {
                        settings.commands = Some(
                            commands
                                .into_iter()
                                .map(|(name, raw)| (name, json_value(raw)))
                                .collect(),
                        );
                    } else {
                        warnings.push(format!(
                            "[commands] in {layer} config ({source}) must be a table — ignored"
                        ));
                    }
                }
                _ => flatten_value(key, value, &mut settings.entries),
            }
        }
        Ok(settings)
    }

    pub(super) fn session(flags: &serde_json::Map<String, Value>) -> Self {
        let mut settings = Self::default();
        for (key, value) in flags {
            if let Some(name) = key.strip_prefix("commands.") {
                settings
                    .commands
                    .get_or_insert_with(Vec::new)
                    .push((name.into(), value.clone()));
            } else {
                settings.entries.push((key.clone(), value.clone()));
            }
        }
        settings
    }
}

fn flatten_value(prefix: String, value: toml::Value, entries: &mut Vec<(String, Value)>) {
    if let toml::Value::Table(table) = value {
        for (key, value) in table {
            flatten_value(format!("{prefix}.{key}"), value, entries);
        }
    } else {
        entries.push((prefix, json_value(value)));
    }
}

fn json_value(value: toml::Value) -> Value {
    // Datetime and non-finite TOML numbers remain non-admissible JSON values.
    serde_json::to_value(value).unwrap_or(Value::Null)
}
