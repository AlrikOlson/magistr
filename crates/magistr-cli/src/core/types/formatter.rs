use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Formatter {
    Rustfmt,
    Prettier,
    Biome,
    Ruff,
    Black,
    Gofmt,
    Rubocop,
    #[serde(rename = "mix-format")]
    MixFormat,
    Swiftformat,
    Ktfmt,
    #[serde(rename = "dotnet-format")]
    DotnetFormat,
    None,
}

impl Formatter {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rustfmt => "rustfmt",
            Self::Prettier => "prettier",
            Self::Biome => "biome",
            Self::Ruff => "ruff",
            Self::Black => "black",
            Self::Gofmt => "gofmt",
            Self::Rubocop => "rubocop",
            Self::MixFormat => "mix-format",
            Self::Swiftformat => "swiftformat",
            Self::Ktfmt => "ktfmt",
            Self::DotnetFormat => "dotnet-format",
            Self::None => "none",
        }
    }
}

impl std::fmt::Display for Formatter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
