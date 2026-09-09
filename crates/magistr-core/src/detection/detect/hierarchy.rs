//! Hierarchy inference — detect parent/child relationships between projects.
//!
//! Sources of hierarchy:
//! - Directory nesting (project A's path is a prefix of project B's path)
//! - Workspace manifests (Cargo workspace members, pnpm-workspace.yaml, lerna.json)
//! - .sln file project references
//! - docker-compose service directories

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::types::{Topology, Workspace};

/// Infer parent/child indices for a list of project paths.
///
/// A project is a child of the nearest ancestor that is also a project.
/// Returns `(parent_indices, children_map)` where `parent_indices[i]` is
/// `Some(j)` if project `j` is the parent of project `i`.
pub fn infer_hierarchy(paths: &[PathBuf]) -> (Vec<Option<usize>>, HashMap<usize, Vec<usize>>) {
    let mut parents: Vec<Option<usize>> = vec![None; paths.len()];
    let mut children: HashMap<usize, Vec<usize>> = HashMap::new();

    for (i, child_path) in paths.iter().enumerate() {
        // Find the nearest ancestor project
        let mut best_parent: Option<(usize, usize)> = None; // (index, depth)

        for (j, parent_path) in paths.iter().enumerate() {
            if i == j {
                continue;
            }
            // Check if parent_path is a proper prefix of child_path
            if is_ancestor(parent_path, child_path) {
                let depth = parent_path.components().count();
                if best_parent.is_none() || depth > best_parent.unwrap().1 {
                    best_parent = Some((j, depth));
                }
            }
        }

        if let Some((parent_idx, _)) = best_parent {
            parents[i] = Some(parent_idx);
            children.entry(parent_idx).or_default().push(i);
        }
    }

    (parents, children)
}

/// Check if `ancestor` path is a proper prefix of `descendant`.
fn is_ancestor(ancestor: &Path, descendant: &Path) -> bool {
    if ancestor.as_os_str().is_empty() {
        // The root project (empty path) is ancestor of everything else
        return !descendant.as_os_str().is_empty();
    }
    descendant.starts_with(ancestor) && descendant != ancestor
}

/// Classify the topology of the project tree.
pub fn classify_topology(
    project_count: usize,
    workspace: Workspace,
    has_nested_children: bool,
) -> Topology {
    if project_count <= 1 {
        return Topology::SingleProject;
    }

    if has_nested_children {
        return Topology::NestedFullstack;
    }

    if workspace != Workspace::None {
        return Topology::Workspace;
    }

    Topology::FlatMonorepo
}

/// Parse Cargo workspace members from a Cargo.toml file.
pub fn parse_cargo_workspace_members(root: &Path) -> Vec<PathBuf> {
    let cargo_path = root.join("Cargo.toml");
    let contents = match std::fs::read_to_string(&cargo_path) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let toml: toml::Value = match toml::from_str(&contents) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut members = Vec::new();
    if let Some(ws) = toml.get("workspace").and_then(|v| v.as_table())
        && let Some(member_list) = ws.get("members").and_then(|v| v.as_array())
    {
        for member in member_list {
            if let Some(s) = member.as_str() {
                // Handle glob patterns (e.g., "crates/*")
                if s.contains('*') {
                    if let Ok(entries) = glob_expand(root, s) {
                        members.extend(entries);
                    }
                } else {
                    let path = PathBuf::from(s);
                    if root.join(&path).exists() {
                        members.push(path);
                    }
                }
            }
        }
    }
    members
}

/// Parse pnpm-workspace.yaml packages.
pub fn parse_pnpm_workspace_packages(root: &Path) -> Vec<PathBuf> {
    let yaml_path = root.join("pnpm-workspace.yaml");
    let contents = match std::fs::read_to_string(&yaml_path) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    // Simple YAML parsing: look for "packages:" section with "- pattern" entries
    let mut packages = Vec::new();
    let mut in_packages = false;

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("packages:") {
            in_packages = true;
            continue;
        }
        if in_packages {
            if !trimmed.starts_with('-') && !trimmed.is_empty() {
                break;
            }
            if let Some(pattern) = trimmed.strip_prefix("- ") {
                let pattern = pattern.trim().trim_matches('\'').trim_matches('"');
                if pattern.contains('*') {
                    if let Ok(entries) = glob_expand(root, pattern) {
                        packages.extend(entries);
                    }
                } else {
                    let path = PathBuf::from(pattern);
                    if root.join(&path).exists() {
                        packages.push(path);
                    }
                }
            }
        }
    }
    packages
}

/// Parse lerna.json packages.
pub fn parse_lerna_packages(root: &Path) -> Vec<PathBuf> {
    let lerna_path = root.join("lerna.json");
    let contents = match std::fs::read_to_string(&lerna_path) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let json: serde_json::Value = match serde_json::from_str(&contents) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut packages = Vec::new();
    if let Some(pkg_array) = json.get("packages").and_then(|v| v.as_array()) {
        for entry in pkg_array {
            if let Some(pattern) = entry.as_str() {
                if pattern.contains('*') {
                    if let Ok(entries) = glob_expand(root, pattern) {
                        packages.extend(entries);
                    }
                } else {
                    let path = PathBuf::from(pattern);
                    if root.join(&path).exists() {
                        packages.push(path);
                    }
                }
            }
        }
    }
    packages
}

/// Parse package.json workspaces field (npm/yarn).
pub fn parse_npm_workspaces(root: &Path) -> Vec<PathBuf> {
    let pkg_path = root.join("package.json");
    let contents = match std::fs::read_to_string(&pkg_path) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let json: serde_json::Value = match serde_json::from_str(&contents) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut packages = Vec::new();

    // "workspaces" can be an array or an object with "packages" key
    let patterns = if let Some(arr) = json.get("workspaces").and_then(|v| v.as_array()) {
        arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>()
    } else if let Some(obj) = json.get("workspaces").and_then(|v| v.as_object()) {
        obj.get("packages")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    for pattern in patterns {
        if pattern.contains('*') {
            if let Ok(entries) = glob_expand(root, pattern) {
                packages.extend(entries);
            }
        } else {
            let path = PathBuf::from(pattern);
            if root.join(&path).exists() {
                packages.push(path);
            }
        }
    }
    packages
}

/// Parse .sln file to extract project paths.
pub fn parse_sln_projects(root: &Path) -> Vec<PathBuf> {
    let mut projects = Vec::new();

    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.ends_with(".sln")
                && let Ok(contents) = std::fs::read_to_string(entry.path())
            {
                // Parse: Project("{...}") = "Name", "Path\To\Project.csproj", "{...}"
                for line in contents.lines() {
                    if let Some(rest) = line.strip_prefix("Project(") {
                        let parts: Vec<&str> = rest.split(',').collect();
                        if parts.len() >= 2 {
                            let path_str = parts[1].trim().trim_matches('"');
                            let path_str = path_str.replace('\\', "/");
                            if let Some(dir) = Path::new(&path_str).parent() {
                                let dir = dir.to_path_buf();
                                if !dir.as_os_str().is_empty() && root.join(&dir).exists() {
                                    projects.push(dir);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    projects.sort();
    projects.dedup();
    projects
}

/// Parse docker-compose.yml to extract service build context directories.
pub fn parse_docker_compose_services(root: &Path) -> Vec<PathBuf> {
    let mut services = Vec::new();

    for name in [
        "docker-compose.yml",
        "docker-compose.yaml",
        "compose.yml",
        "compose.yaml",
    ] {
        let path = root.join(name);
        if let Ok(contents) = std::fs::read_to_string(&path) {
            // Simple YAML parsing: find "build:" values under "services:"
            let mut in_services = false;
            let mut current_indent = 0;

            for line in contents.lines() {
                let trimmed = line.trim();
                let indent = line.len() - line.trim_start().len();

                if trimmed == "services:" {
                    in_services = true;
                    current_indent = indent;
                    continue;
                }

                if in_services && indent <= current_indent && !trimmed.is_empty() {
                    break;
                }

                if in_services {
                    // Look for "build: ./path" or "build:\n  context: ./path"
                    if let Some(build_path) = trimmed.strip_prefix("build: ") {
                        let build_path = build_path.trim().trim_matches('\'').trim_matches('"');
                        let build_path = build_path.strip_prefix("./").unwrap_or(build_path);
                        if !build_path.is_empty() && build_path != "." {
                            let p = PathBuf::from(build_path);
                            if root.join(&p).exists() {
                                services.push(p);
                            }
                        }
                    }
                    if let Some(ctx_path) = trimmed.strip_prefix("context: ") {
                        let ctx_path = ctx_path.trim().trim_matches('\'').trim_matches('"');
                        let ctx_path = ctx_path.strip_prefix("./").unwrap_or(ctx_path);
                        if !ctx_path.is_empty() && ctx_path != "." {
                            let p = PathBuf::from(ctx_path);
                            if root.join(&p).exists() {
                                services.push(p);
                            }
                        }
                    }
                }
            }
        }
    }

    services.sort();
    services.dedup();
    services
}

/// Expand a glob pattern relative to a root directory.
fn glob_expand(root: &Path, pattern: &str) -> Result<Vec<PathBuf>, ()> {
    let full_pattern = root.join(pattern);
    let pattern_str = full_pattern.to_string_lossy();

    let mut results = Vec::new();
    if let Ok(paths) = glob::glob(&pattern_str) {
        for entry in paths.flatten() {
            if entry.is_dir()
                && let Ok(rel) = entry.strip_prefix(root)
            {
                results.push(rel.to_path_buf());
            }
        }
    }
    Ok(results)
}

#[cfg(test)]
#[path = "hierarchy.tests.rs"]
mod tests;
