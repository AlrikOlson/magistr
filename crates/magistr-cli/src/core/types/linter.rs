use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Linter {
    Clippy,
    Eslint,
    Biome,
    Ruff,
    Flake8,
    #[serde(rename = "golangci-lint")]
    GolangciLint,
    Rubocop,
    Credo,
    Swiftlint,
    Ktlint,
    None,
}

impl Linter {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Clippy => "clippy",
            Self::Eslint => "eslint",
            Self::Biome => "biome",
            Self::Ruff => "ruff",
            Self::Flake8 => "flake8",
            Self::GolangciLint => "golangci-lint",
            Self::Rubocop => "rubocop",
            Self::Credo => "credo",
            Self::Swiftlint => "swiftlint",
            Self::Ktlint => "ktlint",
            Self::None => "none",
        }
    }
}

impl std::fmt::Display for Linter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
