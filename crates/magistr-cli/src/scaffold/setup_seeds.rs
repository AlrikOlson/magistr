//! What a first init plants in a fresh configuration beyond its templates.

use magistr_engine::LifecycleHook;

use super::tree_to_config::TreeToConfigResult;

/// The seeds a first init plants in a fresh `.magistr/project.json` and
/// `CLAUDE.md` beyond their templates: the lifecycle hooks the interview or
/// the language preset chose, and the archetypes, topology and relationships
/// a polyglot tree detected.
///
/// The seeds enrich a configuration the setup plan creates. An existing
/// configuration is the user's — re-detection never rewrites its commands,
/// hooks, topology or fields this binary does not know — so the plan leaves
/// it preserved and the seeds untouched.
#[derive(Debug, Default)]
pub struct SetupSeeds {
    /// Hooks bound to workflow phase transitions and events.
    pub lifecycle_hooks: Vec<LifecycleHook>,
    /// The multi-archetype configuration of a polyglot tree.
    pub archetypes: Option<TreeToConfigResult>,
}

impl SetupSeeds {
    /// `generated`, the templated configuration, with the seeds planted; the
    /// template bytes themselves when there is nothing to plant, or when
    /// they are not a configuration this binary can read (an AI override,
    /// say), which keeps whatever the author wrote.
    #[must_use]
    pub(crate) fn project_json(&self, generated: &str) -> String {
        if self.lifecycle_hooks.is_empty() && self.archetypes.is_none() {
            return generated.to_owned();
        }
        let mut config = match serde_json::from_str::<crate::types::ProjectConfig>(generated) {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(target: "magistr", %error, "generated project.json is not a configuration this binary reads; its seeds are not planted");
                return generated.to_owned();
            }
        };
        if !self.lifecycle_hooks.is_empty() {
            config.lifecycle_hooks = self.lifecycle_hooks.clone();
        }
        if let Some(polyglot) = &self.archetypes {
            config.archetypes = polyglot.archetypes.clone();
            config.topology = Some(polyglot.topology);
            config.relationships = polyglot.relationships.clone();
        }
        match serde_json::to_string_pretty(&config) {
            Ok(json) => format!("{json}\n"),
            Err(_) => generated.to_owned(),
        }
    }

    /// `document`, the `CLAUDE.md` the plan writes, with the archetype
    /// section written or refreshed; unchanged without archetypes.
    #[must_use]
    pub(crate) fn claude_md(&self, document: &str) -> String {
        match &self.archetypes {
            Some(polyglot) if !polyglot.archetypes.is_empty() => {
                super::archetype_section::with_archetype_section(
                    document,
                    &polyglot.archetypes,
                    polyglot.topology,
                )
            }
            _ => document.to_owned(),
        }
    }
}

#[cfg(test)]
#[path = "setup_seeds.tests.rs"]
mod tests;
