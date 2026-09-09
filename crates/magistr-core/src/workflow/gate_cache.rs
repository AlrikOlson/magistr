//! Hash-based cache for quality gate results.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::validate::ValidationResult;

/// Source file extensions to consider for cache invalidation.
const SOURCE_EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "kt", "rb", "ex", "exs", "swift", "cpp",
    "c", "h", "hpp", "cs", "toml", "json", "yaml", "yml",
];

/// Hash-based cache for gate results.
///
/// Tracks source file modification times. If no source files have changed
/// since the last gate run, the cached results are returned instead of
/// re-running the gates.
#[derive(Debug, Clone)]
pub struct GateCache {
    /// Hash of source file mtimes from the last successful cache fill.
    source_hash: Option<u64>,
    /// Cached validation results.
    /// Last stored gate result (regardless of cache validity).
    pub last_result: Option<ValidationResult>,
}

impl GateCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self {
            source_hash: None,
            last_result: None,
        }
    }

    /// Check if the cache is still valid for the given project directory.
    ///
    /// Computes a hash of source file modification times and compares
    /// against the stored hash. Returns `true` if the hashes match.
    pub fn is_valid(&self, project_dir: &Path) -> bool {
        match self.source_hash {
            Some(stored_hash) => {
                let current_hash = compute_source_hash(project_dir);
                current_hash == stored_hash
            }
            None => false,
        }
    }

    /// Store gate results in the cache.
    pub fn store(&mut self, project_dir: &Path, result: ValidationResult) {
        self.source_hash = Some(compute_source_hash(project_dir));
        self.last_result = Some(result);
    }

    /// Retrieve cached results, if valid.
    pub fn get(&self, project_dir: &Path) -> Option<&ValidationResult> {
        if self.is_valid(project_dir) {
            self.last_result.as_ref()
        } else {
            None
        }
    }

    /// Invalidate the cache.
    pub fn invalidate(&mut self) {
        self.source_hash = None;
        self.last_result = None;
    }

    /// Whether the cache has stored results (regardless of validity).
    pub fn has_results(&self) -> bool {
        self.last_result.is_some()
    }
}

impl Default for GateCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute a hash from source file modification times.
///
/// Uses a fast approach: collects (path, mtime) pairs for all source files,
/// sorts them for determinism, then hashes the result. This is much faster
/// than content-hashing every file.
fn compute_source_hash(project_dir: &Path) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut entries: BTreeMap<PathBuf, u64> = BTreeMap::new();
    collect_source_mtimes(project_dir, project_dir, &mut entries);

    let mut hasher = DefaultHasher::new();
    for (path, mtime) in &entries {
        path.hash(&mut hasher);
        mtime.hash(&mut hasher);
    }
    // Also hash the entry count to differentiate empty from missing
    entries.len().hash(&mut hasher);

    hasher.finish()
}

/// Recursively collect source file modification times.
fn collect_source_mtimes(root: &Path, dir: &Path, entries: &mut BTreeMap<PathBuf, u64>) {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        // Skip hidden dirs, target/, node_modules/, etc.
        if name.starts_with('.')
            || name == "target"
            || name == "node_modules"
            || name == "dist"
            || name == "build"
            || name == "__pycache__"
        {
            continue;
        }

        if path.is_dir() {
            collect_source_mtimes(root, &path, entries);
        } else if is_source_extension(&path)
            && let Ok(meta) = path.metadata()
        {
            let mtime = meta
                .modified()
                .unwrap_or(SystemTime::UNIX_EPOCH)
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_nanos() as u64;
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            entries.insert(relative, mtime);
        }
    }
}

/// Check if a file has a source code extension.
fn is_source_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| SOURCE_EXTENSIONS.contains(&ext))
}

#[cfg(test)]
#[path = "gate_cache.tests.rs"]
mod tests;
