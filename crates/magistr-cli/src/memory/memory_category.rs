//! Category of a memory entry.

use serde::{Deserialize, Serialize};

/// Category of a memory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryCategory {
    /// Preferred code patterns (e.g., "Uses builder pattern for config structs")
    Patterns,
    /// Architecture decisions (e.g., "Monorepo with shared types crate")
    Architecture,
    /// Team/project conventions (e.g., "All errors use thiserror")
    Conventions,
    /// Common mistakes to avoid (e.g., "Don't use unwrap() in library code")
    Mistakes,
    /// Session observations — automatically appended, not curated
    Observations,
}

impl MemoryCategory {
    /// All categories in display order.
    pub const ALL: &'static [Self] = &[
        Self::Patterns,
        Self::Architecture,
        Self::Conventions,
        Self::Mistakes,
        Self::Observations,
    ];

    /// Human-readable heading for markdown sections.
    #[must_use]
    pub fn heading(&self) -> &'static str {
        match self {
            Self::Patterns => "Patterns",
            Self::Architecture => "Architecture",
            Self::Conventions => "Conventions",
            Self::Mistakes => "Common Mistakes",
            Self::Observations => "Observations",
        }
    }

    /// Parse a heading string back to a category.
    pub fn from_heading(s: &str) -> Option<Self> {
        match s.trim() {
            "Patterns" => Some(Self::Patterns),
            "Architecture" => Some(Self::Architecture),
            "Conventions" => Some(Self::Conventions),
            "Common Mistakes" => Some(Self::Mistakes),
            "Observations" => Some(Self::Observations),
            _ => None,
        }
    }
}

impl std::fmt::Display for MemoryCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.heading())
    }
}

#[cfg(test)]
#[path = "memory_category.tests.rs"]
mod tests;
