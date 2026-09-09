//! Cross-archetype conflict detection — detect when agents in different
//! archetypes make conflicting changes.
//!
//! Detects: API contract changes that break consumers, shared type
//! modifications, and incompatible dependency version bumps. Uses
//! relationship edges to identify potential conflict zones.

use std::path::{Path, PathBuf};

use magistr_core::types::{ArchetypeRelationship, RelationshipKind};

use super::topology::AgentId;

/// A detected cross-archetype conflict.
#[derive(Debug, Clone)]
pub struct ArchetypeConflict {
    /// The file or pattern involved in the conflict.
    pub file: PathBuf,
    /// IDs of archetypes involved.
    pub archetype_ids: Vec<String>,
    /// Agent IDs that modified conflicting files.
    pub agent_ids: Vec<AgentId>,
    /// Kind of cross-archetype conflict.
    pub kind: ConflictKind,
}

/// Classification of cross-archetype conflicts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictKind {
    /// An API contract file was modified by the provider archetype.
    /// Consumers may need to adapt.
    ApiContractChanged,
    /// A shared type definition was modified. Both archetypes may
    /// need coordinated updates.
    SharedTypeModified,
    /// A dependency version was changed in one archetype that may
    /// conflict with another archetype's constraints.
    DependencyVersionConflict,
}

impl std::fmt::Display for ConflictKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ApiContractChanged => write!(f, "API contract changed"),
            Self::SharedTypeModified => write!(f, "shared type modified"),
            Self::DependencyVersionConflict => write!(f, "dependency version conflict"),
        }
    }
}

/// Files that typically define API contracts or shared interfaces.
const API_CONTRACT_FILES: &[&str] = &[
    "openapi.json",
    "openapi.yaml",
    "openapi.yml",
    "swagger.json",
    "swagger.yaml",
    "schema.graphql",
    "schema.prisma",
];

/// Files that commonly define shared types.
const SHARED_TYPE_FILES: &[&str] = &[
    "types.ts",
    "types.d.ts",
    "types.rs",
    "models.rs",
    "models.ts",
    "shared.rs",
    "contracts.rs",
    "interfaces.ts",
];

/// Files that define dependency versions.
const DEPENDENCY_FILES: &[&str] = &[
    "package.json",
    "Cargo.toml",
    "requirements.txt",
    "pyproject.toml",
    "go.mod",
    "Gemfile",
    "composer.json",
    "build.gradle",
    "pom.xml",
];

/// Detect cross-archetype conflicts from a list of files modified by agents.
///
/// For each modified file, checks if it's in a "conflict zone" defined by the
/// relationship edges — e.g., if a backend agent modifies an API contract file
/// and the frontend depends on the backend, that's a conflict.
#[must_use]
pub fn detect_cross_archetype_conflicts(
    modified_files: &[(AgentId, String, PathBuf)], // (agent_id, archetype_id, file)
    relationships: &[ArchetypeRelationship],
) -> Vec<ArchetypeConflict> {
    let mut conflicts = Vec::new();

    for (agent_id, arch_id, file) in modified_files {
        let filename = file
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        // Check if this file is in a conflict zone.
        let conflict_kind = classify_conflict_file(&filename);
        let Some(kind) = conflict_kind else {
            continue;
        };

        // Find archetypes that have a relationship with this one.
        let affected_archetypes: Vec<String> = relationships
            .iter()
            .filter_map(|rel| {
                if &rel.to == arch_id && rel.kind == RelationshipKind::DependsOn {
                    Some(rel.from.clone()) // dependent is affected
                } else if (&rel.from == arch_id || &rel.to == arch_id)
                    && rel.kind == RelationshipKind::SharesWith
                {
                    let other = if &rel.from == arch_id {
                        &rel.to
                    } else {
                        &rel.from
                    };
                    Some(other.clone())
                } else {
                    None
                }
            })
            .collect();

        if !affected_archetypes.is_empty() {
            let mut all_archetypes = vec![arch_id.clone()];
            all_archetypes.extend(affected_archetypes);

            conflicts.push(ArchetypeConflict {
                file: file.clone(),
                archetype_ids: all_archetypes,
                agent_ids: vec![*agent_id],
                kind,
            });
        }
    }

    conflicts
}

/// Classify a file as a potential cross-archetype conflict source.
fn classify_conflict_file(filename: &str) -> Option<ConflictKind> {
    let matches = |patterns: &[&str]| {
        patterns
            .iter()
            .any(|p| filename.contains(&p.to_lowercase()))
    };
    if matches(API_CONTRACT_FILES) {
        Some(ConflictKind::ApiContractChanged)
    } else if matches(SHARED_TYPE_FILES) {
        Some(ConflictKind::SharedTypeModified)
    } else if matches(DEPENDENCY_FILES) {
        Some(ConflictKind::DependencyVersionConflict)
    } else {
        None
    }
}

/// Check if a path is in a conflict zone for the given relationships.
#[must_use]
pub fn is_in_conflict_zone(path: &Path, _relationships: &[ArchetypeRelationship]) -> bool {
    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    classify_conflict_file(&filename).is_some()
}

#[cfg(test)]
#[path = "archetype_conflict.tests.rs"]
mod tests;
