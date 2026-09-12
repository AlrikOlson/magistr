use serde::{Deserialize, Serialize};

/// Which store backs the project roadmap.
///
/// `ThinkAndShip` (the default since ts5a) reads chunks from the native
/// engines in [`crate::native`]; `Yaml` is the legacy `.magistr-roadmap.yml` format,
/// still selectable and inferred for projects that predate the key (see
/// `magistr_core::roadmap::backend_for`). Wire form is the project.json value:
/// `"roadmap_backend": "yaml" | "think-and-ship"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoadmapBackend {
    Yaml,
    #[default]
    ThinkAndShip,
}

impl RoadmapBackend {
    /// Whether the native think-and-ship engines back the roadmap.
    #[must_use]
    pub const fn is_native(self) -> bool {
        matches!(self, Self::ThinkAndShip)
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
            Self::ThinkAndShip => "think-and-ship",
        }
    }
}

#[cfg(test)]
#[path = "roadmap_backend.tests.rs"]
mod tests;
