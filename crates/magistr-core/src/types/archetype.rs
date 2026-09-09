use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::project_role::ProjectRole;
use super::quality_gate::{FormatConfig, QualityGate, TestingConfig};

/// A single archetype within a polyglot project.
///
/// Each archetype represents one language/framework sub-project with its own
/// build commands, lint rules, test runners, file patterns, and conventions.
/// In a single-language project there is exactly one archetype; in a monorepo
/// there may be many (e.g. a Rust backend + a React frontend).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Archetype {
    /// Unique slug identifier (e.g. `"backend"`, `"web-frontend"`).
    pub id: String,
    /// Human-readable display name (e.g. `"Rust Backend"`).
    pub name: String,
    /// Primary language (e.g. `"rust"`, `"typescript"`).
    pub language: String,
    /// Framework, if any (e.g. `"nextjs"`, `"django"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub framework: Option<String>,
    /// Root path relative to the repository root (e.g. `"."`, `"apps/web"`).
    pub root: PathBuf,
    /// Role within the repository.
    pub role: ProjectRole,
    /// Quality gate commands for this archetype.
    pub quality_gate: QualityGate,
    /// Code formatter configuration.
    pub format: FormatConfig,
    /// Test runner and file placement configuration.
    pub testing: TestingConfig,
    /// Source file glob patterns (e.g. `["src/**/*.rs"]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_patterns: Vec<String>,
    /// Test file glob patterns (e.g. `["tests/**/*.rs", "src/**/*.tests.rs"]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub test_patterns: Vec<String>,
    /// Config file glob patterns (e.g. `["Cargo.toml", ".cargo/**"]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub config_patterns: Vec<String>,
    /// Naming convention (e.g. `"snake_case"`, `"camelCase"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub naming_convention: Option<String>,
    /// Module structure convention (e.g. `"one-pub-per-file"`, `"barrel-exports"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_structure: Option<String>,
    /// Preferred model for agents working in this archetype.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_preference: Option<String>,
}

/// A directed relationship between two archetypes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchetypeRelationship {
    /// Source archetype ID.
    pub from: String,
    /// Target archetype ID.
    pub to: String,
    /// Kind of relationship.
    pub kind: RelationshipKind,
}

/// How two archetypes relate to each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RelationshipKind {
    /// Source depends on target (e.g. frontend imports types from shared lib).
    DependsOn,
    /// Source hosts target (e.g. backend serves frontend build output).
    Hosts,
    /// Source and target share code (e.g. shared types package).
    SharesWith,
}

/// Resolve which archetype a file path belongs to.
///
/// Matches by root path prefix — the archetype whose root is the longest
/// prefix of `path` wins. Falls back to glob matching on source/test/config
/// patterns if no root matches.
#[must_use]
pub fn resolve_archetype<'a>(archetypes: &'a [Archetype], path: &Path) -> Option<&'a Archetype> {
    if archetypes.is_empty() {
        return None;
    }
    if archetypes.len() == 1 {
        return Some(&archetypes[0]);
    }

    // Longest root-prefix match wins.
    let mut best: Option<(usize, &Archetype)> = None;
    for arch in archetypes {
        let root = &arch.root;
        // Normalize: "." root matches everything.
        let matches = if root == Path::new(".") {
            true
        } else {
            path.starts_with(root)
        };
        if matches {
            let depth = if root == Path::new(".") {
                0
            } else {
                root.components().count()
            };
            if best.is_none_or(|(d, _)| depth > d) {
                best = Some((depth, arch));
            }
        }
    }
    best.map(|(_, a)| a)
}

#[cfg(test)]
#[path = "archetype.tests.rs"]
mod tests;
