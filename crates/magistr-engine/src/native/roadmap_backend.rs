use serde::{Deserialize, Serialize};

/// Which store backs the project roadmap.
///
/// `Native` (the default since ts5a) reads chunks from the native
/// engines in [`crate::native`]; `Yaml` explicitly selects `.magistr-roadmap.yml`.
/// An omitted project setting uses the native default. Wire form:
/// `"roadmap_backend": "yaml" | "native"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoadmapBackend {
    Yaml,
    #[default]
    Native,
}

impl RoadmapBackend {
    /// Whether the native magistr engines back the roadmap.
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
#[path = "roadmap_backend.tests.rs"]
mod tests;
