use std::path::Path;

use crate::types::Language;

/// Extract the project name from manifest files, falling back to the directory name.
pub fn detect_project_name(dir: &Path, lang: Language) -> String {
    let from_manifest = match lang {
        Language::Rust => extract_from_cargo_toml(dir),
        Language::TypeScript | Language::JavaScript => extract_from_package_json(dir),
        Language::Python => extract_from_pyproject_toml(dir),
        Language::Go => extract_from_go_mod(dir),
        _ => None,
    };

    from_manifest.unwrap_or_else(|| {
        dir.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "project".to_string())
    })
}

fn extract_from_cargo_toml(dir: &Path) -> Option<String> {
    let contents = std::fs::read_to_string(dir.join("Cargo.toml")).ok()?;
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with("name") {
            // Parse: name = "value"
            if let Some(val) = line.split('=').nth(1) {
                let val = val.trim().trim_matches('"');
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

fn extract_from_package_json(dir: &Path) -> Option<String> {
    let contents = std::fs::read_to_string(dir.join("package.json")).ok()?;
    // Simple JSON parsing — find "name": "value"
    let parsed: serde_json::Value = serde_json::from_str(&contents).ok()?;
    parsed
        .get("name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn extract_from_pyproject_toml(dir: &Path) -> Option<String> {
    let contents = std::fs::read_to_string(dir.join("pyproject.toml")).ok()?;
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with("name")
            && let Some(val) = line.split('=').nth(1)
        {
            let val = val.trim().trim_matches('"');
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

fn extract_from_go_mod(dir: &Path) -> Option<String> {
    let contents = std::fs::read_to_string(dir.join("go.mod")).ok()?;
    let first_line = contents.lines().next()?;
    // Format: module github.com/user/repo
    let module_path = first_line.strip_prefix("module")?.trim();
    // Take the last path segment
    module_path.rsplit('/').next().map(|s| s.to_string())
}

#[cfg(test)]
#[path = "project_name.tests.rs"]
mod tests;
