use serde::{Deserialize, Serialize};

/// The inferred role of a detected project within a repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectRole {
    /// SPA or static frontend (React, Vue, Svelte, Angular, etc.).
    Frontend,
    /// Server-side application (Express, ASP.NET, Actix, Django, etc.).
    Backend,
    /// Project with both frontend and backend characteristics.
    Fullstack,
    /// Reusable library/package (workspace member with no binary target).
    Library,
    /// Command-line tool (has clap, commander, argparse, etc.).
    Cli,
    /// Service referenced in docker-compose or similar orchestration.
    Service,
    /// Role could not be determined.
    Unknown,
}

impl ProjectRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Frontend => "frontend",
            Self::Backend => "backend",
            Self::Fullstack => "fullstack",
            Self::Library => "library",
            Self::Cli => "cli",
            Self::Service => "service",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for ProjectRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
