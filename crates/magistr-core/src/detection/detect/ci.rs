use std::path::Path;

use crate::types::CiProvider;

/// Detect the CI/CD provider from project root.
///
/// Checks for well-known CI configuration files and directories:
/// - `.github/workflows/` → GitHub Actions
/// - `.gitlab-ci.yml` → GitLab CI
/// - `.circleci/` → CircleCI
/// - `.travis.yml` → Travis CI
pub fn detect_ci_provider(dir: &Path) -> CiProvider {
    // GitHub Actions: any YAML file in .github/workflows/
    let workflows_dir = dir.join(".github/workflows");
    if workflows_dir.is_dir()
        && let Ok(entries) = std::fs::read_dir(&workflows_dir)
    {
        let has_yaml = entries.filter_map(|e| e.ok()).any(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.ends_with(".yml") || name.ends_with(".yaml")
        });
        if has_yaml {
            return CiProvider::GitHubActions;
        }
    }

    // GitLab CI
    if dir.join(".gitlab-ci.yml").exists() {
        return CiProvider::GitLabCi;
    }

    // CircleCI
    if dir.join(".circleci").join("config.yml").exists() {
        return CiProvider::CircleCi;
    }

    // Travis CI
    if dir.join(".travis.yml").exists() {
        return CiProvider::TravisCi;
    }

    CiProvider::None
}

#[cfg(test)]
#[path = "ci.tests.rs"]
mod tests;
