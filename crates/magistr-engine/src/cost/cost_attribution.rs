use serde::{Deserialize, Serialize};

use super::budget_scope::BudgetScope;

/// Who and what a unit of spend is charged to.
///
/// This is the half of a cost observation that becomes *labels*: every
/// field here ends up as a metric attribute, so each one has a defined
/// value even when the run could not resolve it. An unresolved actor
/// reports as `unattributed` rather than dropping the series, matching
/// how `magistr costs --group-by user` already buckets unattributed ledger
/// entries — a missing dimension must never make spend disappear.
///
/// `provider` is an OpenTelemetry `gen_ai.provider.name` value, which is
/// a closed enum. Magistr's default provider is Claude Code over ACP, so
/// the default here is `anthropic`; a provider that talks to something
/// else must say so rather than let the label lie.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostAttribution {
    /// The team identity this spend is charged to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    /// The model the request was made to (`gen_ai.request.model`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The GenAI provider (`gen_ai.provider.name`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// The project the run was against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The scope whose headroom bound this run's budget, when one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<BudgetScope>,
}

impl CostAttribution {
    /// The `gen_ai.provider.name` used when a backend names none.
    pub const DEFAULT_PROVIDER: &'static str = "anthropic";
    /// The actor label for spend that resolved no identity.
    pub const UNATTRIBUTED: &'static str = "unattributed";
    /// The label used for any other dimension the run could not resolve.
    pub const UNKNOWN: &'static str = "unknown";

    /// Charge this spend to a team identity (self-consuming builder).
    #[must_use]
    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }

    /// Attribute this spend to a model (self-consuming builder).
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Name the GenAI provider (self-consuming builder).
    #[must_use]
    pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    /// Attribute this spend to a project (self-consuming builder).
    #[must_use]
    pub fn with_project(mut self, project: impl Into<String>) -> Self {
        self.project = Some(project.into());
        self
    }

    /// Record which scope's headroom bound the budget (self-consuming builder).
    #[must_use]
    pub fn with_scope(mut self, scope: BudgetScope) -> Self {
        self.scope = Some(scope);
        self
    }

    /// The actor label — never empty.
    #[must_use]
    pub fn actor_label(&self) -> &str {
        label_or(self.actor.as_deref(), Self::UNATTRIBUTED)
    }

    /// The `gen_ai.request.model` label — never empty.
    #[must_use]
    pub fn model_label(&self) -> &str {
        label_or(self.model.as_deref(), Self::UNKNOWN)
    }

    /// The `gen_ai.provider.name` label — never empty.
    #[must_use]
    pub fn provider_label(&self) -> &str {
        label_or(self.provider.as_deref(), Self::DEFAULT_PROVIDER)
    }

    /// The project label — never empty.
    #[must_use]
    pub fn project_label(&self) -> &str {
        label_or(self.project.as_deref(), Self::UNKNOWN)
    }

    /// The binding-scope label. `unbound` means no scope took the
    /// budget below what the developer asked for — which is a real
    /// answer, not a missing one.
    #[must_use]
    pub fn scope_label(&self) -> &'static str {
        self.scope.as_ref().map_or("unbound", BudgetScope::as_str)
    }
}

/// A trimmed non-empty value, or `fallback`.
fn label_or<'a>(value: Option<&'a str>, fallback: &'a str) -> &'a str {
    match value.map(str::trim) {
        Some(v) if !v.is_empty() => v,
        _ => fallback,
    }
}

#[cfg(test)]
#[path = "cost_attribution.tests.rs"]
mod tests;
