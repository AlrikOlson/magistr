use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Rust,
    #[serde(alias = "typescript")]
    TypeScript,
    #[serde(alias = "javascript")]
    JavaScript,
    Python,
    Go,
    Java,
    Kotlin,
    Ruby,
    Elixir,
    Swift,
    #[serde(alias = "cpp")]
    Cpp,
    #[serde(alias = "csharp")]
    CSharp,
    Dart,
    #[serde(alias = "php")]
    Php,
    Scala,
    Unknown,
}

impl Language {
    /// All known language variants (excluding `Unknown`).
    pub const ALL: &[Language] = &[
        Self::Rust,
        Self::TypeScript,
        Self::JavaScript,
        Self::Python,
        Self::Go,
        Self::Java,
        Self::Kotlin,
        Self::Ruby,
        Self::Elixir,
        Self::Swift,
        Self::Cpp,
        Self::CSharp,
        Self::Dart,
        Self::Php,
        Self::Scala,
    ];

    /// Parse a language name loosely, returning `None` for unrecognized names.
    ///
    /// Accepts the same strings as `as_str()` returns, case-insensitive.
    /// Does not return `Unknown` — use this when you need to validate
    /// that a name maps to a real built-in preset.
    pub fn from_str_loose(s: &str) -> Option<Self> {
        let lower = s.trim().to_lowercase();
        Self::ALL
            .iter()
            .find(|lang| lang.as_str() == lower)
            .copied()
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::Python => "python",
            Self::Go => "go",
            Self::Java => "java",
            Self::Kotlin => "kotlin",
            Self::Ruby => "ruby",
            Self::Elixir => "elixir",
            Self::Swift => "swift",
            Self::Cpp => "cpp",
            Self::CSharp => "csharp",
            Self::Dart => "dart",
            Self::Php => "php",
            Self::Scala => "scala",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
