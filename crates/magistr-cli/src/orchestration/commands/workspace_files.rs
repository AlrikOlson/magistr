//! The workspace file list the cockpit's `@`-completion offers.

use std::path::Path;

/// How many files the list carries at most — plenty for a completion
/// popup, a bound for a huge workspace.
const CAP: usize = 2_000;

/// Directories that never hold files worth mentioning in a prompt.
const SKIPPED: [&str; 3] = ["target", "node_modules", "vendor"];

/// The workspace's files under `root`, workspace-relative with `/`
/// separators, sorted, capped at [`CAP`]. Hidden entries (a leading
/// `.`) and the build/vendor trees are skipped; an unreadable directory
/// is skipped rather than an error — the list feeds a completion popup,
/// not an audit.
#[must_use]
pub fn workspace_files(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    collect(root, root, &mut files);
    files.sort();
    files.truncate(CAP);
    files
}

fn collect(root: &Path, dir: &Path, files: &mut Vec<String>) {
    if files.len() >= CAP {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if files.len() >= CAP {
            return;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.starts_with('.') || SKIPPED.contains(&name) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, files);
        } else if let Ok(relative) = path.strip_prefix(root) {
            files.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
}

#[cfg(test)]
#[path = "workspace_files.tests.rs"]
mod tests;
