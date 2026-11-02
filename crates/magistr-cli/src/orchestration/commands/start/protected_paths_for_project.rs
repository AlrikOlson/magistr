use std::path::PathBuf;

/// Build the list of protected file paths for a project.
///
/// These files are managed by MCP tools and must not be edited directly.
/// Currently includes the plan YAML file when it exists.
pub fn protected_paths_for_project(project_dir: &std::path::Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Protect the plan YAML file
    let yaml_path = crate::plan::yaml_path(project_dir);
    if yaml_path.exists() {
        paths.push(yaml_path);
    }

    paths
}
