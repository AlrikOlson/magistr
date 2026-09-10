use std::path::{Component, Path, PathBuf};

/// Resolve a diagnostic path against the session's root, following existing
/// symlinks before `..` and resolving missing children through their real
/// ancestors. Unrooted relative paths stay relative and are normalized lexically.
/// An inaccessible target keeps its original spelling; authorization uses the
/// checked form and refuses resolution errors instead of granting that fallback.
#[must_use]
pub fn resolve_against(root: Option<&Path>, path: &Path) -> PathBuf {
    checked_resolve_against(root, path).unwrap_or_else(|_| joined(root, path))
}

/// The policy must distinguish an unresolved target from a usable path.
pub(crate) fn checked_resolve_against(root: Option<&Path>, path: &Path) -> Result<PathBuf, String> {
    let joined = joined(root, path);
    if joined.is_relative() {
        // Never resolve against the process cwd, which can be another project.
        return Ok(normalize(&joined));
    }
    let anchor = joined
        .ancestors()
        .last()
        .expect("an absolute path has a root");
    magistr_acp::capabilities::resolve_target(anchor, &joined).map_err(|error| error.to_string())
}

fn joined(root: Option<&Path>, path: &Path) -> PathBuf {
    match root {
        Some(root) if path.is_relative() => root.join(path),
        _ => path.to_path_buf(),
    }
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let popped =
                    matches!(out.components().next_back(), Some(Component::Normal(_))) && out.pop();
                if !popped && !out.has_root() {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
#[path = "resolve_against.tests.rs"]
mod tests;
