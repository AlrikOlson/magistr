//! The cockpit's chrome inputs, read once at an entry point.

use std::path::Path;

/// The header's session title (`magistr — project · backend · model`,
/// the parts that are known) and whether the diagnostics chrome is on
/// (`MAGISTR_TUI_DEBUG` set to anything). The environment is read here,
/// at the entry point — the cockpit core never touches it.
pub fn chrome(root: &Path, backend: Option<&str>, model: Option<&str>) -> (String, bool) {
    let project = root.file_name().map_or_else(
        || root.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let mut title = format!("magistr — {project}");
    for part in [backend, model].into_iter().flatten() {
        title.push_str(" · ");
        title.push_str(part);
    }
    (title, std::env::var_os("MAGISTR_TUI_DEBUG").is_some())
}

#[cfg(test)]
#[path = "chrome.tests.rs"]
mod tests;
