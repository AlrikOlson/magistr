//! Magistr workspace paths and source-selection exclusions.
use std::path::{Component, Path};

/// Repository metadata and workspace runtime state are never source inputs.
/// Match components so nested state is excluded without hiding project settings.
pub(crate) fn is_owner_path(path: &Path) -> bool {
    let mut magistr = false;
    for part in path.components() {
        if let Component::Normal(name) = part {
            let name = name.to_string_lossy();
            if name.eq_ignore_ascii_case(".git")
                || (magistr && name.eq_ignore_ascii_case("workspace"))
            {
                return true;
            }
            magistr = name.eq_ignore_ascii_case(".magistr");
        } else {
            magistr = false;
        }
    }
    false
}

#[cfg(test)]
#[path = "paths.tests.rs"]
mod tests;
