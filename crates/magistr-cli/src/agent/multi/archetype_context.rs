//! Cross-archetype context injection — inject dependency context across archetypes.
//!
//! When an agent working on archetype A needs to understand archetype B's
//! contracts (e.g., API response types, shared interfaces, GraphQL schema),
//! this module detects cross-archetype dependencies from the relationship
//! edges and builds context sections to inject into agent prompts.

use std::path::Path;

use magistr_core::types::{Archetype, ArchetypeRelationship, RelationshipKind};

/// Files that commonly define cross-archetype contracts.
const CONTRACT_PATTERNS: &[&str] = &[
    "openapi.json",
    "openapi.yaml",
    "openapi.yml",
    "swagger.json",
    "swagger.yaml",
    "schema.graphql",
    "schema.prisma",
    "types.ts",
    "types.d.ts",
    "shared.rs",
    "contracts.rs",
    "api.rs",
    "models.rs",
    "proto",
];

/// Build a context injection section for a given archetype based on its relationships.
///
/// Scans the relationships to find archetypes that the target depends on or
/// shares code with, then identifies contract files in those archetypes that
/// should be injected as context.
#[must_use]
pub fn build_cross_archetype_context(
    target_archetype: &Archetype,
    all_archetypes: &[Archetype],
    relationships: &[ArchetypeRelationship],
    project_root: &Path,
) -> String {
    let mut sections = Vec::new();

    for rel in relationships {
        // Find relationships where target depends on or shares with another archetype.
        let related_id = if rel.from == target_archetype.id {
            match rel.kind {
                RelationshipKind::DependsOn | RelationshipKind::SharesWith => Some(&rel.to),
                RelationshipKind::Hosts => None,
            }
        } else if rel.to == target_archetype.id {
            match rel.kind {
                RelationshipKind::DependsOn => None, // Reverse dep — the other depends on us.
                RelationshipKind::SharesWith => Some(&rel.from),
                RelationshipKind::Hosts => Some(&rel.from),
            }
        } else {
            None
        };

        let Some(related_id) = related_id else {
            continue;
        };

        let Some(related_arch) = all_archetypes.iter().find(|a| &a.id == related_id) else {
            continue;
        };

        // Find contract files in the related archetype.
        let contract_files = find_contract_files(related_arch, project_root);
        if contract_files.is_empty() {
            continue;
        }

        let file_list = contract_files
            .iter()
            .map(|f| format!("  - `{}`", f.display()))
            .collect::<Vec<_>>()
            .join("\n");

        sections.push(format!(
            "### Contracts from {name} ({rel_kind})\n\n\
             The `{name}` archetype ({lang}) has the following contract files \
             relevant to your work:\n{file_list}",
            name = related_arch.name,
            rel_kind = relationship_label(&rel.kind),
            lang = related_arch.language,
        ));
    }

    if sections.is_empty() {
        String::new()
    } else {
        format!(
            "## Cross-Archetype Context\n\n\
             The following contract files from related archetypes may be relevant. \
             Read them to understand shared types, API contracts, and interfaces.\n\n\
             {}",
            sections.join("\n\n")
        )
    }
}

/// Find contract/interface files in an archetype's directory.
fn find_contract_files(archetype: &Archetype, project_root: &Path) -> Vec<std::path::PathBuf> {
    let arch_dir = project_root.join(&archetype.root);
    if !arch_dir.is_dir() {
        return Vec::new();
    }

    let mut found = Vec::new();
    scan_for_contracts(&arch_dir, &arch_dir, &mut found, 3);
    found
}

/// Recursively scan up to `max_depth` for contract-like files.
fn scan_for_contracts(
    dir: &Path,
    base: &Path,
    found: &mut Vec<std::path::PathBuf>,
    max_depth: u32,
) {
    if max_depth == 0 {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if path.is_dir() {
            // Skip common noise directories.
            if matches!(
                name_str.as_ref(),
                "node_modules" | "target" | ".git" | "dist" | "build" | "__pycache__"
            ) {
                continue;
            }
            scan_for_contracts(&path, base, found, max_depth - 1);
        } else if is_contract_file(&name_str) {
            // Store relative to the base directory.
            if let Ok(relative) = path.strip_prefix(base) {
                found.push(relative.to_path_buf());
            }
        }
    }
}

/// Check if a filename matches known contract file patterns.
fn is_contract_file(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    CONTRACT_PATTERNS.iter().any(|p| lower.contains(p))
}

/// Human-readable label for a relationship kind.
fn relationship_label(kind: &RelationshipKind) -> &'static str {
    match kind {
        RelationshipKind::DependsOn => "dependency",
        RelationshipKind::Hosts => "hosted by",
        RelationshipKind::SharesWith => "shared code",
    }
}

#[cfg(test)]
#[path = "archetype_context.tests.rs"]
mod tests;
