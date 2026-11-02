use serde::{Deserialize, Serialize};

/// Which store backs the project plan.
///
/// `Native` (the default since ts5a) reads tasks from the native
/// engines in [`crate::native`]; `Yaml` explicitly selects `.magistr-plan.yml`.
/// An omitted project setting uses the native default. Wire form:
/// `"plan_backend": "yaml" | "native"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlanBackend {
    Yaml,
    #[default]
    Native,
}

impl PlanBackend {
    /// Whether the native magistr engines back the plan.
    #[must_use]
    pub const fn is_native(self) -> bool {
        matches!(self, Self::Native)
    }

    /// Whether this is the default (used to elide the field on write).
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The project.json wire form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Yaml => "yaml",
            Self::Native => "native",
        }
    }
}

#[cfg(test)]
#[path = "plan_backend.tests.rs"]
mod tests;
