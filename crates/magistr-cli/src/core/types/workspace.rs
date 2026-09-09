use serde::{Deserialize, Serialize};

/// Workspace/monorepo type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Workspace {
    Cargo,
    Npm,
    Pnpm,
    Yarn,
    Nx,
    Turborepo,
    None,
}

impl Workspace {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cargo => "cargo",
            Self::Npm => "npm",
            Self::Pnpm => "pnpm",
            Self::Yarn => "yarn",
            Self::Nx => "nx",
            Self::Turborepo => "turborepo",
            Self::None => "none",
        }
    }
}

impl std::fmt::Display for Workspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
