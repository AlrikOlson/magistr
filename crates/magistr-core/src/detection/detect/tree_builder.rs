//! Tree builder — orchestrates scanner → per-project detection → hierarchy →
//! relationships → role classification into a complete [`ProjectTree`].

use std::collections::HashSet;
use std::path::Path;

use crate::types::{DetectedProject, Framework, ProjectMetadata, ProjectTree, Workspace};

use super::hierarchy;
use super::relationships;
use super::role;
use super::scanner;

/// Build a complete [`ProjectTree`] for a repository.
///
/// Scans the directory for marker files, runs per-project detection on each
/// discovered project directory, infers hierarchy and relationships, classifies
/// roles, and assembles the result.
pub fn build_project_tree(root: &Path) -> crate::error::Result<ProjectTree> {
    let root = root
        .canonicalize()
        .map_err(|e| crate::error::MagistrError::io(root, e))?;

    // Step 1: Scan for markers
    let found = scanner::scan_directory(&root);
    let project_dirs = scanner::unique_project_dirs(&found);

    if project_dirs.is_empty() {
        return Ok(ProjectTree::new(root));
    }

    // Step 2: Detect workspace type at root
    let workspace = super::workspace::detect_workspace(&root);

    // Step 3: Gather workspace member lists for enrichment
    let workspace_members = collect_workspace_members(&root, workspace);

    // Step 4: Gather docker-compose service directories
    let docker_services = hierarchy::parse_docker_compose_services(&root);
    let docker_service_set: HashSet<_> = docker_services.iter().collect();

    // Step 5: Run per-project detection + role classification
    let mut projects: Vec<DetectedProject> = Vec::new();

    for (rel_path, markers) in &project_dirs {
        // Skip workspace-only markers (no project boundary)
        let has_project_marker = markers.iter().any(|m| m.is_project_boundary());
        if !has_project_marker && !rel_path.as_os_str().is_empty() {
            continue;
        }

        projects.extend(detect_projects_for_dir(
            &root,
            rel_path,
            markers,
            &docker_service_set,
        ));
    }

    // Also add workspace members that weren't found by the scanner
    for member_path in &workspace_members {
        // Normalize: "." is equivalent to "" (root)
        let normalized = if member_path.as_os_str() == "." {
            std::path::PathBuf::new()
        } else {
            member_path.clone()
        };
        if !projects.iter().any(|p| p.path == normalized) {
            let abs_dir = root.join(member_path);
            if abs_dir.exists() {
                projects.extend(detect_projects_for_dir(
                    &root,
                    &normalized,
                    &[],
                    &docker_service_set,
                ));
            }
        }
    }

    // Step 6: Infer hierarchy
    let paths: Vec<_> = projects.iter().map(|p| p.path.clone()).collect();
    let (parents, children_map) = hierarchy::infer_hierarchy(&paths);

    for (i, parent_idx) in parents.iter().enumerate() {
        projects[i].parent = *parent_idx;
    }
    for (parent_idx, child_indices) in &children_map {
        projects[*parent_idx].children = child_indices.clone();
    }

    // Step 7: Detect relationships (edges)
    let mut tree = ProjectTree::new(root.clone());
    tree.projects = projects;
    tree.workspace = workspace;

    let path_index = tree.path_index();
    // Convert from HashMap<&Path, usize> to work with the root
    let edges = relationships::detect_relationships(&root, &path_index);
    tree.edges = edges;

    // Step 8: Classify topology
    // "has_nested_children" = any project with children is itself a non-root
    // project (e.g., an API with a nested SPA). Root having children is just
    // a workspace or monorepo. But if workspace == None and root has children
    // that are actual projects (not workspace members), it's nested fullstack.
    let has_nested = if workspace == Workspace::None {
        // Without a workspace manager, root having children means nesting
        tree.projects.iter().any(|p| !p.children.is_empty())
    } else {
        // With a workspace, nesting means a non-root project has children
        tree.projects
            .iter()
            .any(|p| p.parent.is_some() && !p.children.is_empty())
    };
    tree.topology = hierarchy::classify_topology(tree.len(), workspace, has_nested);

    Ok(tree)
}

/// Build one [`DetectedProject`] per language evidenced in a directory.
///
/// A directory carrying manifests for several stacks (`Cargo.toml` AND
/// `package.json`) is a polyglot boundary, not a project of whichever stack
/// the detection ladder reaches first: each evidenced language becomes its own
/// project, primary language first, so no stack's gates are shadowed by
/// another's. A directory with no evidenced language keeps today's single
/// Unknown-language project.
fn detect_projects_for_dir(
    root: &Path,
    rel_path: &std::path::PathBuf,
    markers: &[crate::types::StackMarker],
    docker_service_set: &HashSet<&std::path::PathBuf>,
) -> Vec<DetectedProject> {
    let abs_dir = root.join(rel_path);

    // Collect key dependencies from manifest (all stacks' manifests, capped)
    let manifest = super::config_parser::parse_manifests(&abs_dir);
    let key_deps: Vec<String> = manifest.dependencies.iter().take(20).cloned().collect();
    let is_docker_service = docker_service_set.contains(rel_path);

    let languages = super::language::detect_languages(&abs_dir);
    let stacks: Vec<crate::types::Stack> = if languages.is_empty() {
        vec![super::detect_stack(&abs_dir).unwrap_or_default()]
    } else {
        languages
            .iter()
            .map(|lang| super::detect_stack_as(&abs_dir, *lang).unwrap_or_default())
            .collect()
    };

    stacks
        .into_iter()
        .map(|stack| {
            let framework = if stack.framework == Framework::None {
                None
            } else {
                Some(stack.framework)
            };
            let project_role = role::classify_role(
                &abs_dir,
                stack.language,
                framework,
                markers,
                &key_deps,
                is_docker_service,
            );
            DetectedProject {
                path: rel_path.clone(),
                language: stack.language,
                framework,
                role: project_role,
                markers: markers.to_vec(),
                metadata: ProjectMetadata {
                    name: if stack.project_name.is_empty() {
                        None
                    } else {
                        Some(stack.project_name)
                    },
                    dependencies: key_deps.clone(),
                    version: None,
                },
                parent: None,
                children: vec![],
            }
        })
        .collect()
}

/// Collect workspace member paths from the detected workspace type.
fn collect_workspace_members(root: &Path, workspace: Workspace) -> Vec<std::path::PathBuf> {
    match workspace {
        Workspace::Cargo => hierarchy::parse_cargo_workspace_members(root),
        Workspace::Pnpm => hierarchy::parse_pnpm_workspace_packages(root),
        Workspace::Npm | Workspace::Yarn => hierarchy::parse_npm_workspaces(root),
        Workspace::Lerna => hierarchy::parse_lerna_packages(root),
        Workspace::DotnetSln => hierarchy::parse_sln_projects(root),
        Workspace::Nx | Workspace::Turborepo | Workspace::Rush | Workspace::Moon => {
            // These tools typically use package.json workspaces or their own config
            // Fall back to npm workspace parsing if available
            hierarchy::parse_npm_workspaces(root)
        }
        Workspace::None => Vec::new(),
    }
}

#[cfg(test)]
#[path = "tree_builder.tests.rs"]
mod tests;
