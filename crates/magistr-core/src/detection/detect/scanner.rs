//! Recursive filesystem scanner that collects marker files in a repository.
//!
//! Respects `.gitignore` rules and configurable ignore patterns. Returns
//! discovered markers sorted by depth (shallowest first) then priority
//! (most specific first).

use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

use crate::types::StackMarker;

/// A marker file found during scanning.
#[derive(Debug, Clone)]
pub struct FoundMarker {
    /// Path to the directory containing the marker, relative to the scan root.
    pub dir: PathBuf,
    /// The marker file found.
    pub marker: StackMarker,
    /// Depth from the scan root (0 = root directory).
    pub depth: usize,
}

/// Default directory names to skip during scanning.
const DEFAULT_IGNORE_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "bin",
    "obj",
    "__pycache__",
    ".venv",
    "venv",
    ".git",
    ".hg",
    "dist",
    "build",
    ".next",
    ".nuxt",
    "vendor",
    ".tox",
    "coverage",
    ".cache",
];

/// Scan a directory recursively for stack marker files.
///
/// Uses the `ignore` crate to respect `.gitignore` rules. Additionally skips
/// common build/dependency directories listed in [`DEFAULT_IGNORE_DIRS`].
///
/// Returns markers sorted by depth (shallowest first), then by marker priority
/// (most specific first within the same depth).
pub fn scan_directory(root: &Path) -> Vec<FoundMarker> {
    scan_directory_with_ignores(root, DEFAULT_IGNORE_DIRS)
}

/// Scan with custom ignore patterns (for testing or configuration).
pub fn scan_directory_with_ignores(root: &Path, ignore_dirs: &[&str]) -> Vec<FoundMarker> {
    let mut markers = Vec::new();

    let walker = WalkBuilder::new(root)
        .hidden(false) // don't skip hidden files (we handle .git ourselves)
        .git_ignore(true) // respect .gitignore
        .git_global(false) // skip global gitignore for determinism
        .git_exclude(true) // respect .git/info/exclude
        .max_depth(Some(10)) // reasonable depth limit
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        // Only look at files, not directories
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let file_name = match entry.file_name().to_str() {
            Some(name) => name,
            None => continue,
        };

        // Check if this file matches any stack marker
        let marker = match StackMarker::from_filename(file_name) {
            Some(m) => m,
            None => continue,
        };

        // Get the containing directory relative to root
        let abs_path = entry.path();
        let parent = match abs_path.parent() {
            Some(p) => p,
            None => continue,
        };

        // Skip ignored directories
        let rel_path = match parent.strip_prefix(root) {
            Ok(p) => p,
            Err(_) => continue,
        };

        if should_skip(rel_path, ignore_dirs) {
            continue;
        }

        let depth = entry.depth();

        markers.push(FoundMarker {
            dir: rel_path.to_path_buf(),
            marker,
            depth,
        });
    }

    // Sort by depth (shallowest first), then by priority (lowest = most specific)
    markers.sort_by(|a, b| {
        a.depth
            .cmp(&b.depth)
            .then(a.marker.priority().cmp(&b.marker.priority()))
    });

    markers
}

/// Check if a relative path contains any of the ignored directory names.
fn should_skip(rel_path: &Path, ignore_dirs: &[&str]) -> bool {
    for component in rel_path.components() {
        if let std::path::Component::Normal(name) = component {
            let name = name.to_string_lossy();
            if ignore_dirs.iter().any(|&ig| ig == &*name) {
                return true;
            }
        }
    }
    false
}

/// Deduplicate markers to unique project directories.
///
/// Returns one entry per unique directory, keeping the highest-priority
/// (lowest weight) marker when multiple markers exist in the same directory.
/// Additional markers for the same directory are collected as secondary markers.
pub fn unique_project_dirs(markers: &[FoundMarker]) -> Vec<(PathBuf, Vec<StackMarker>)> {
    use std::collections::BTreeMap;

    let mut dirs: BTreeMap<&Path, Vec<StackMarker>> = BTreeMap::new();

    for fm in markers {
        dirs.entry(&fm.dir).or_default().push(fm.marker);
    }

    // Deduplicate markers within each directory
    let mut result: Vec<(PathBuf, Vec<StackMarker>)> = dirs
        .into_iter()
        .map(|(path, mut markers)| {
            markers.sort_by_key(|m| m.priority());
            markers.dedup();
            (path.to_path_buf(), markers)
        })
        .collect();

    // Sort by path depth (shallowest first)
    result.sort_by(|a, b| {
        let depth_a = a.0.components().count();
        let depth_b = b.0.components().count();
        depth_a.cmp(&depth_b)
    });

    result
}

#[cfg(test)]
#[path = "scanner.tests.rs"]
mod tests;
