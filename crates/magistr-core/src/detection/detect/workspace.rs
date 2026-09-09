use std::path::Path;

use crate::types::Workspace;

/// Detect workspace/monorepo type from project root.
///
/// Checks for workspace configuration in manifest files:
/// - Cargo.toml `[workspace]` → Cargo workspace
/// - turbo.json → Turborepo (checked before npm workspaces since it layers on top)
/// - nx.json → Nx
/// - pnpm-workspace.yaml → pnpm workspaces
/// - package.json `"workspaces"` → npm/yarn workspaces
pub fn detect_workspace(dir: &Path) -> Workspace {
    // Cargo workspace: [workspace] section in Cargo.toml
    if std::fs::read_to_string(dir.join("Cargo.toml"))
        .is_ok_and(|contents| contents.contains("[workspace]"))
    {
        return Workspace::Cargo;
    }

    // Turborepo: turbo.json at root (check before generic npm workspaces)
    if dir.join("turbo.json").exists() {
        return Workspace::Turborepo;
    }

    // Nx: nx.json at root
    if dir.join("nx.json").exists() {
        return Workspace::Nx;
    }

    // pnpm workspaces: pnpm-workspace.yaml
    if dir.join("pnpm-workspace.yaml").exists() {
        return Workspace::Pnpm;
    }

    // Lerna: lerna.json at root
    if dir.join("lerna.json").exists() {
        return Workspace::Lerna;
    }

    // Moon: moon.yml at root
    if dir.join("moon.yml").exists() {
        return Workspace::Moon;
    }

    // .NET solution: *.sln at root
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().ends_with(".sln") {
                return Workspace::DotnetSln;
            }
        }
    }

    // npm/yarn workspaces: "workspaces" field in package.json
    if std::fs::read_to_string(dir.join("package.json"))
        .is_ok_and(|contents| contents.contains("\"workspaces\""))
    {
        if dir.join("yarn.lock").exists() {
            return Workspace::Yarn;
        }
        return Workspace::Npm;
    }

    Workspace::None
}

#[cfg(test)]
#[path = "workspace.tests.rs"]
mod tests;
