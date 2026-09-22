//! Bridge from [`ProjectTree`] detection to multi-archetype [`ProjectConfig`] fields.
//!
//! Converts a detected project tree into the archetype, topology, and relationship
//! data that populates `project.json`. Each detected project becomes one `Archetype`
//! with quality gate, format, and testing config derived from the language preset.

use std::collections::HashSet;
use std::path::PathBuf;

use magistr_core::types::{
    Archetype, ArchetypeRelationship, EdgeKind, FormatConfig, ProjectTree, QualityGate,
    RelationshipKind, TestingConfig, Topology,
};

/// Result of converting a `ProjectTree` into config-ready data.
#[derive(Debug)]
pub struct TreeToConfigResult {
    /// One archetype per detected project.
    pub archetypes: Vec<Archetype>,
    /// Repository topology.
    pub topology: Topology,
    /// Inter-archetype relationships derived from project edges.
    pub relationships: Vec<ArchetypeRelationship>,
}

/// Convert a `ProjectTree` into archetypes, topology, and relationships.
///
/// For each `DetectedProject`, loads the language/framework preset — with the
/// user's override from `user_preset_dir` when the caller captured one — to
/// populate quality gate, format, and testing config. Derives a unique slug
/// id from the project name or path.
pub fn tree_to_archetypes(
    tree: &ProjectTree,
    user_preset_dir: Option<&std::path::Path>,
) -> TreeToConfigResult {
    let mut archetypes: Vec<Archetype> = Vec::with_capacity(tree.projects.len());
    let mut used_ids: HashSet<String> = HashSet::new();

    for project in &tree.projects {
        let (id, name) = derive_id_and_name(project, &used_ids);
        used_ids.insert(id.clone());

        let lang = project.language;
        let framework = project
            .framework
            .unwrap_or(magistr_core::types::Framework::None);
        let abs_dir = tree.root.join(&project.path);
        let pkg_mgr = magistr_core::detection::detect::detect_package_manager(&abs_dir, lang);

        let (quality_gate, format, testing) =
            load_preset_config(&abs_dir, lang, framework, pkg_mgr, user_preset_dir);

        let framework_str = project.framework.map(|f| f.as_str().to_string());

        archetypes.push(Archetype {
            id,
            name,
            language: lang.as_str().to_string(),
            framework: framework_str,
            root: if project.path.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                project.path.clone()
            },
            role: project.role,
            quality_gate,
            format,
            testing,
            source_patterns: source_patterns_for(lang),
            test_patterns: test_patterns_for(lang),
            config_patterns: vec![],
            naming_convention: naming_convention_for(lang),
            module_structure: None,
            model_preference: None,
        });
    }

    let relationships = edges_to_relationships(tree, &archetypes);

    TreeToConfigResult {
        archetypes,
        topology: tree.topology,
        relationships,
    }
}

/// Derive a unique id and display name from a detected project.
fn derive_id_and_name(
    project: &magistr_core::types::DetectedProject,
    used_ids: &HashSet<String>,
) -> (String, String) {
    let raw_name = project
        .metadata
        .name
        .as_deref()
        .filter(|n| !n.is_empty())
        .or_else(|| {
            // Use the last path component as the name
            project
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .filter(|n| !n.is_empty())
        });

    let base = match raw_name {
        Some(n) => slug(n),
        None => "default".to_string(),
    };

    let display_name = raw_name.unwrap_or("default").to_string();

    // Ensure uniqueness by appending a suffix if needed
    if !used_ids.contains(&base) {
        return (base, display_name);
    }

    // A polyglot dir yields twins sharing a name — the language is what
    // tells them apart, so try it before a bare number.
    let lang_suffixed = format!("{base}-{}", project.language.as_str());
    if !used_ids.contains(&lang_suffixed) {
        return (lang_suffixed, display_name);
    }

    for i in 2..=99 {
        let candidate = format!("{base}-{i}");
        if !used_ids.contains(&candidate) {
            return (candidate, display_name);
        }
    }

    // Fallback: use the full path
    let fallback = slug(&project.path.to_string_lossy());
    (fallback, display_name)
}

/// Convert a name to a URL-safe slug.
fn slug(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Load quality gate, format, and testing config from the preset for this language/framework.
fn load_preset_config(
    dir: &std::path::Path,
    lang: magistr_core::types::Language,
    framework: magistr_core::types::Framework,
    pkg_mgr: magistr_core::types::PackageManager,
    user_preset_dir: Option<&std::path::Path>,
) -> (QualityGate, FormatConfig, TestingConfig) {
    let layers = magistr_core::config::preset::LayerOptions {
        user_preset_dir: user_preset_dir.map(Into::into),
        project_root: Some(dir.into()),
        ..Default::default()
    };
    match magistr_core::config::preset::load_preset_layered(lang, framework, pkg_mgr, &layers) {
        Ok(preset) => (preset.quality_gate, preset.format, preset.testing),
        Err(_) => {
            let fallback = magistr_core::config::preset::fallback_preset();
            (fallback.quality_gate, fallback.format, fallback.testing)
        }
    }
}

/// Convert project edges to archetype relationships.
fn edges_to_relationships(
    tree: &ProjectTree,
    archetypes: &[Archetype],
) -> Vec<ArchetypeRelationship> {
    tree.edges
        .iter()
        .filter_map(|edge| {
            let from_id = archetypes.get(edge.from).map(|a| a.id.clone())?;
            let to_id = archetypes.get(edge.to).map(|a| a.id.clone())?;
            let kind = match edge.kind {
                EdgeKind::CargoPathDep | EdgeKind::NpmWorkspaceRef | EdgeKind::CsprojRef => {
                    RelationshipKind::DependsOn
                }
                EdgeKind::SharedDir => RelationshipKind::SharesWith,
            };
            Some(ArchetypeRelationship {
                from: from_id,
                to: to_id,
                kind,
            })
        })
        .collect()
}

/// Default source patterns for a language.
fn source_patterns_for(lang: magistr_core::types::Language) -> Vec<String> {
    use magistr_core::types::Language;
    match lang {
        Language::Rust => vec!["src/**/*.rs".into()],
        Language::TypeScript => vec!["src/**/*.ts".into(), "src/**/*.tsx".into()],
        Language::JavaScript => vec!["src/**/*.js".into(), "src/**/*.jsx".into()],
        Language::Python => vec!["**/*.py".into()],
        Language::Go => vec!["**/*.go".into()],
        Language::Java => vec!["src/**/*.java".into()],
        Language::Kotlin => vec!["src/**/*.kt".into()],
        Language::Ruby => vec!["lib/**/*.rb".into(), "app/**/*.rb".into()],
        Language::CSharp => vec!["**/*.cs".into()],
        Language::Swift => vec!["Sources/**/*.swift".into()],
        Language::Dart => vec!["lib/**/*.dart".into()],
        Language::Php => vec!["src/**/*.php".into()],
        _ => vec![],
    }
}

/// Default test patterns for a language.
fn test_patterns_for(lang: magistr_core::types::Language) -> Vec<String> {
    use magistr_core::types::Language;
    match lang {
        Language::Rust => vec!["tests/**/*.rs".into(), "src/**/*.tests.rs".into()],
        Language::TypeScript => vec![
            "**/*.test.ts".into(),
            "**/*.spec.ts".into(),
            "__tests__/**/*.ts".into(),
        ],
        Language::JavaScript => vec![
            "**/*.test.js".into(),
            "**/*.spec.js".into(),
            "__tests__/**/*.js".into(),
        ],
        Language::Python => vec!["tests/**/*.py".into(), "test_*.py".into()],
        Language::Go => vec!["**/*_test.go".into()],
        Language::Java => vec!["src/test/**/*.java".into()],
        Language::Ruby => vec!["spec/**/*.rb".into(), "test/**/*.rb".into()],
        Language::CSharp => vec!["**/*Tests.cs".into()],
        Language::Swift => vec!["Tests/**/*.swift".into()],
        Language::Dart => vec!["test/**/*.dart".into()],
        Language::Php => vec!["tests/**/*.php".into()],
        _ => vec![],
    }
}

/// Default naming convention for a language.
fn naming_convention_for(lang: magistr_core::types::Language) -> Option<String> {
    use magistr_core::types::Language;
    match lang {
        Language::Rust | Language::Python | Language::Ruby | Language::Elixir => {
            Some("snake_case".into())
        }
        Language::TypeScript
        | Language::JavaScript
        | Language::Java
        | Language::Kotlin
        | Language::Dart
        | Language::Php
        | Language::Scala
        | Language::Swift => Some("camelCase".into()),
        Language::Go => Some("camelCase".into()),
        Language::CSharp => Some("PascalCase".into()),
        _ => None,
    }
}

#[cfg(test)]
#[path = "tree_to_config.tests.rs"]
mod tests;
