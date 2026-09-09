use super::*;
use crate::types::CiProvider;
use tempfile::TempDir;

#[test]
fn detect_github_actions() {
    let dir = TempDir::new().unwrap();
    let workflows = dir.path().join(".github/workflows");
    std::fs::create_dir_all(&workflows).unwrap();
    std::fs::write(workflows.join("ci.yml"), "name: CI").unwrap();

    assert_eq!(detect_ci_provider(dir.path()), CiProvider::GitHubActions);
}

#[test]
fn detect_github_actions_yaml_extension() {
    let dir = TempDir::new().unwrap();
    let workflows = dir.path().join(".github/workflows");
    std::fs::create_dir_all(&workflows).unwrap();
    std::fs::write(workflows.join("build.yaml"), "name: Build").unwrap();

    assert_eq!(detect_ci_provider(dir.path()), CiProvider::GitHubActions);
}

#[test]
fn detect_github_actions_empty_workflows_dir() {
    let dir = TempDir::new().unwrap();
    let workflows = dir.path().join(".github/workflows");
    std::fs::create_dir_all(&workflows).unwrap();
    // No YAML files

    assert_eq!(detect_ci_provider(dir.path()), CiProvider::None);
}

#[test]
fn detect_gitlab_ci() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join(".gitlab-ci.yml"), "stages:\n  - test").unwrap();

    assert_eq!(detect_ci_provider(dir.path()), CiProvider::GitLabCi);
}

#[test]
fn detect_circleci() {
    let dir = TempDir::new().unwrap();
    let circleci = dir.path().join(".circleci");
    std::fs::create_dir_all(&circleci).unwrap();
    std::fs::write(circleci.join("config.yml"), "version: 2.1").unwrap();

    assert_eq!(detect_ci_provider(dir.path()), CiProvider::CircleCi);
}

#[test]
fn detect_travis_ci() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join(".travis.yml"), "language: rust").unwrap();

    assert_eq!(detect_ci_provider(dir.path()), CiProvider::TravisCi);
}

#[test]
fn detect_no_ci() {
    let dir = TempDir::new().unwrap();

    assert_eq!(detect_ci_provider(dir.path()), CiProvider::None);
}

#[test]
fn github_actions_takes_priority_over_travis() {
    let dir = TempDir::new().unwrap();
    let workflows = dir.path().join(".github/workflows");
    std::fs::create_dir_all(&workflows).unwrap();
    std::fs::write(workflows.join("ci.yml"), "name: CI").unwrap();
    std::fs::write(dir.path().join(".travis.yml"), "language: rust").unwrap();

    assert_eq!(detect_ci_provider(dir.path()), CiProvider::GitHubActions);
}
