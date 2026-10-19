use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::infra::PersistenceConfig;

use super::{
    ConfigSource,
    config::{OutputFormat, ThinkConfig},
};

/// Frozen reasoning settings resolved from explicit host inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThinkConfigResolution {
    pub config: ThinkConfig,
    pub sources: BTreeMap<&'static str, ConfigSource>,
}

impl ThinkConfigResolution {
    /// Read only the supplied lookup. Automatic session identity is supplied by
    /// the admitting host after it has selected the project.
    #[must_use]
    pub fn from_lookup(
        get: impl Fn(&str) -> Option<String>,
        persistence: PersistenceConfig,
    ) -> Self {
        let mut config = ThinkConfig::with_persistence(persistence);
        let mut sources = [
            "validation.require_thought_prefix",
            "validation.require_rationale_prefix",
            "validation.allow_custom_purpose",
            "validation.strict_mode",
            "features.enable_revisions",
            "features.enable_branching",
            "features.enable_confidence",
            "features.enable_structured_actions",
            "features.enable_sessions",
            "features.default_session_id",
            "display.color_output",
            "display.output_format",
            "system.max_history_size",
            "system.max_branch_depth",
            "system.session_timeout",
            "system.recent_steps_limit",
            "system.rollup_max_items",
            "broadcast.path",
        ]
        .into_iter()
        .map(|field| (field, ConfigSource::Default))
        .collect::<BTreeMap<_, _>>();
        sources.insert("persistence.enabled", ConfigSource::Explicit);
        sources.insert("persistence.data_dir", ConfigSource::Explicit);

        const STRICT: &str = "MAGISTR_WORKFLOW_STRICT_MODE";
        if get(STRICT).as_deref() == Some("true") {
            config.validation.strict_mode = true;
            config.validation.require_thought_prefix = true;
            config.validation.require_rationale_prefix = true;
            config.validation.allow_custom_purpose = false;
            for field in [
                "validation.strict_mode",
                "validation.require_thought_prefix",
                "validation.require_rationale_prefix",
                "validation.allow_custom_purpose",
            ] {
                sources.insert(field, ConfigSource::Environment(STRICT));
            }
        }

        integer(
            &get,
            "MAX_HISTORY_SIZE",
            "system.max_history_size",
            &mut config.system.max_history_size,
            &mut sources,
        );
        integer(
            &get,
            "MAGISTR_WORKFLOW_SESSION_TIMEOUT",
            "system.session_timeout",
            &mut config.system.session_timeout,
            &mut sources,
        );
        integer(
            &get,
            "MAGISTR_WORKFLOW_MAX_BRANCH_DEPTH",
            "system.max_branch_depth",
            &mut config.system.max_branch_depth,
            &mut sources,
        );
        integer(
            &get,
            "MAGISTR_WORKFLOW_RECENT_STEPS_LIMIT",
            "system.recent_steps_limit",
            &mut config.system.recent_steps_limit,
            &mut sources,
        );
        integer(
            &get,
            "MAGISTR_WORKFLOW_ROLLUP_MAX_ITEMS",
            "system.rollup_max_items",
            &mut config.system.rollup_max_items,
            &mut sources,
        );

        const FORMAT: &str = "MAGISTR_WORKFLOW_OUTPUT_FORMAT";
        if let Some(raw) = get(FORMAT) {
            if let Some(format) = OutputFormat::parse(&raw) {
                config.display.output_format = format;
                sources.insert("display.output_format", ConfigSource::Environment(FORMAT));
            } else {
                tracing::warn!(
                    "Invalid MAGISTR_WORKFLOW_OUTPUT_FORMAT; using console. Expected console, json, or markdown"
                );
            }
        }

        const NO_COLOR: &str = "MAGISTR_WORKFLOW_NO_COLOR";
        if get(NO_COLOR).as_deref() == Some("true") {
            config.display.color_output = false;
            sources.insert("display.color_output", ConfigSource::Environment(NO_COLOR));
        }

        const BROADCAST: &str = "MAGISTR_WORKFLOW_BROADCAST_PATH";
        if let Some(raw) = get(BROADCAST) {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                config.broadcast.path = Some(PathBuf::from(trimmed));
                sources.insert("broadcast.path", ConfigSource::Environment(BROADCAST));
            }
        }

        // Both names currently select the same setting; neither is protected.
        // The admitting host supplies identity to avoid another ambient read.
        for variable in [
            "MAGISTR_WORKFLOW_ENABLE_SESSIONS",
            "MAGISTR_WORKFLOW_AUTO_SESSION",
        ] {
            if get(variable).as_deref() == Some("true") {
                config.features.enable_sessions = true;
                sources.insert(
                    "features.enable_sessions",
                    ConfigSource::Environment(variable),
                );
                sources.insert("features.default_session_id", ConfigSource::ProjectIdentity);
            }
        }
        const SESSION_ID: &str = "MAGISTR_WORKFLOW_DEFAULT_SESSION_ID";
        if let Some(raw) = get(SESSION_ID) {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                // An empty override preserves the project-derived default.
            } else if is_safe_session_id(trimmed) {
                config.features.enable_sessions = true;
                config.features.default_session_id = Some(trimmed.to_owned());
                sources.insert(
                    "features.enable_sessions",
                    ConfigSource::Environment(SESSION_ID),
                );
                sources.insert(
                    "features.default_session_id",
                    ConfigSource::Environment(SESSION_ID),
                );
            } else {
                tracing::warn!(
                    "Ignoring invalid MAGISTR_WORKFLOW_DEFAULT_SESSION_ID; expected [A-Za-z0-9_.-], at most 128 characters, excluding . and .."
                );
            }
        }

        Self { config, sources }
    }
}

/// Parse the retained integer settings: whitespace is ignored, and values
/// below one or outside the setting's integer range keep the built-in default.
fn integer<T: std::str::FromStr + PartialOrd + From<u8>>(
    get: &impl Fn(&str) -> Option<String>,
    variable: &'static str,
    field: &'static str,
    target: &mut T,
    sources: &mut BTreeMap<&'static str, ConfigSource>,
) {
    let Some(value) = get(variable).and_then(|raw| raw.trim().parse::<T>().ok()) else {
        return;
    };
    if value >= T::from(1) {
        *target = value;
        sources.insert(field, ConfigSource::Environment(variable));
    }
}

/// Same allowlist as persistence; configuration rejects unsafe values before
/// any downstream persistence sees them.
fn is_safe_session_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        && id != "."
        && id != ".."
}

#[cfg(test)]
#[path = "config_resolution.tests.rs"]
mod tests;
