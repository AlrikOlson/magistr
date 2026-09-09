//! Inter-project relationship detection.
//!
//! Detects dependency edges between projects in a repository:
//! - Cargo workspace path dependencies
//! - npm/pnpm/yarn workspace references
//! - .csproj `<ProjectReference>` elements
//! - Shared directory detection

use std::collections::HashMap;
use std::path::Path;

use crate::types::{EdgeKind, ProjectEdge};

/// Detect inter-project dependency edges.
///
/// Takes a path index (project path → index in the tree) and the absolute
/// root path. Returns edges representing detected dependencies.
pub fn detect_relationships(root: &Path, path_index: &HashMap<&Path, usize>) -> Vec<ProjectEdge> {
    let mut edges = Vec::new();

    for (&project_path, &project_idx) in path_index {
        let abs_dir = root.join(project_path);

        // Cargo path dependencies
        detect_cargo_path_deps(&abs_dir, root, project_idx, path_index, &mut edges);

        // npm workspace references
        detect_npm_workspace_refs(&abs_dir, root, project_idx, path_index, &mut edges);

        // .csproj ProjectReference
        detect_csproj_refs(&abs_dir, root, project_idx, path_index, &mut edges);
    }

    // Deduplicate edges
    edges.sort_by(|a, b| a.from.cmp(&b.from).then(a.to.cmp(&b.to)));
    edges.dedup_by(|a, b| a.from == b.from && a.to == b.to);

    edges
}

/// Detect Cargo path dependencies from `[dependencies]` sections.
fn detect_cargo_path_deps(
    dir: &Path,
    root: &Path,
    from_idx: usize,
    path_index: &HashMap<&Path, usize>,
    edges: &mut Vec<ProjectEdge>,
) {
    let cargo_path = dir.join("Cargo.toml");
    let contents = match std::fs::read_to_string(&cargo_path) {
        Ok(s) => s,
        Err(_) => return,
    };
    let toml: toml::Value = match toml::from_str(&contents) {
        Ok(v) => v,
        Err(_) => return,
    };

    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(deps) = toml.get(section).and_then(|v| v.as_table()) {
            for value in deps.values() {
                let path_str = if let Some(tbl) = value.as_table() {
                    tbl.get("path").and_then(|v| v.as_str())
                } else {
                    None
                };

                if let Some(path_str) = path_str {
                    let dep_abs = dir.join(path_str);
                    if let Ok(dep_abs) = dep_abs.canonicalize()
                        && let Ok(rel) = dep_abs.strip_prefix(root)
                        && let Some(&to_idx) = path_index.get(rel)
                        && to_idx != from_idx
                    {
                        edges.push(ProjectEdge {
                            from: from_idx,
                            to: to_idx,
                            kind: EdgeKind::CargoPathDep,
                        });
                    }
                }
            }
        }
    }
}

/// Detect npm workspace references in package.json dependencies.
fn detect_npm_workspace_refs(
    dir: &Path,
    root: &Path,
    from_idx: usize,
    path_index: &HashMap<&Path, usize>,
    edges: &mut Vec<ProjectEdge>,
) {
    let pkg_path = dir.join("package.json");
    let contents = match std::fs::read_to_string(&pkg_path) {
        Ok(s) => s,
        Err(_) => return,
    };
    let json: serde_json::Value = match serde_json::from_str(&contents) {
        Ok(v) => v,
        Err(_) => return,
    };

    // Build a name → index map from all projects' package.json
    let mut name_to_idx: HashMap<String, usize> = HashMap::new();
    for (&path, &idx) in path_index {
        let other_pkg = root.join(path).join("package.json");
        if let Ok(other_contents) = std::fs::read_to_string(&other_pkg)
            && let Ok(other_json) = serde_json::from_str::<serde_json::Value>(&other_contents)
            && let Some(name) = other_json.get("name").and_then(|v| v.as_str())
        {
            name_to_idx.insert(name.to_string(), idx);
        }
    }

    // Check if any dependency matches a sibling project name
    for section in ["dependencies", "devDependencies", "peerDependencies"] {
        if let Some(deps) = json.get(section).and_then(|v| v.as_object()) {
            for (dep_name, version) in deps {
                // workspace: protocol or matching name
                let is_workspace_ref = version
                    .as_str()
                    .is_some_and(|v| v.starts_with("workspace:"));

                if (is_workspace_ref || name_to_idx.contains_key(dep_name))
                    && let Some(&to_idx) = name_to_idx.get(dep_name)
                    && to_idx != from_idx
                {
                    edges.push(ProjectEdge {
                        from: from_idx,
                        to: to_idx,
                        kind: EdgeKind::NpmWorkspaceRef,
                    });
                }
            }
        }
    }
}

/// Detect .csproj `<ProjectReference>` elements.
fn detect_csproj_refs(
    dir: &Path,
    root: &Path,
    from_idx: usize,
    path_index: &HashMap<&Path, usize>,
    edges: &mut Vec<ProjectEdge>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.ends_with(".csproj") {
            continue;
        }

        let contents = match std::fs::read_to_string(entry.path()) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // Simple XML parsing: find <ProjectReference Include="...">
        for line in contents.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("<ProjectReference Include=\"")
                && let Some(end) = rest.find('"')
            {
                let ref_path = &rest[..end];
                let ref_path = ref_path.replace('\\', "/");
                let ref_abs = dir.join(&ref_path);
                if let Ok(ref_abs) = ref_abs.canonicalize()
                    && let Some(ref_dir) = ref_abs.parent()
                    && let Ok(rel) = ref_dir.strip_prefix(root)
                    && let Some(&to_idx) = path_index.get(rel)
                    && to_idx != from_idx
                {
                    edges.push(ProjectEdge {
                        from: from_idx,
                        to: to_idx,
                        kind: EdgeKind::CsprojRef,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "relationships.tests.rs"]
mod tests;
