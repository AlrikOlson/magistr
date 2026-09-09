use std::path::Path;

use crate::types::{Language, PackageManager};

/// Detect the package manager based on language and lock files.
pub fn detect_package_manager(dir: &Path, lang: Language) -> PackageManager {
    match lang {
        Language::Rust => PackageManager::Cargo,
        Language::TypeScript | Language::JavaScript => {
            if dir.join("pnpm-lock.yaml").exists() {
                PackageManager::Pnpm
            } else if dir.join("yarn.lock").exists() {
                PackageManager::Yarn
            } else if dir.join("bun.lockb").exists() || dir.join("bun.lock").exists() {
                PackageManager::Bun
            } else {
                PackageManager::Npm
            }
        }
        Language::Python => {
            if dir.join("uv.lock").exists() {
                PackageManager::Uv
            } else if dir.join("poetry.lock").exists() {
                PackageManager::Poetry
            } else if dir.join("pyproject.toml").exists() {
                if std::fs::read_to_string(dir.join("pyproject.toml"))
                    .is_ok_and(|contents| contents.contains("[tool.poetry]"))
                {
                    return PackageManager::Poetry;
                }
                PackageManager::Pip
            } else if dir.join("Pipfile").exists() {
                PackageManager::Pipenv
            } else {
                PackageManager::Pip
            }
        }
        Language::Go => PackageManager::Go,
        Language::Java => {
            if dir.join("pom.xml").exists() {
                PackageManager::Maven
            } else {
                PackageManager::Gradle
            }
        }
        Language::Kotlin => PackageManager::Gradle,
        Language::Ruby => PackageManager::Bundler,
        Language::Elixir => PackageManager::Mix,
        Language::Swift => PackageManager::Spm,
        Language::Cpp => {
            if dir.join("CMakeLists.txt").exists() {
                PackageManager::Cmake
            } else if dir.join("meson.build").exists() {
                PackageManager::Meson
            } else {
                PackageManager::Make
            }
        }
        Language::CSharp => PackageManager::Dotnet,
        Language::Dart => PackageManager::Pub,
        Language::Php => PackageManager::Composer,
        Language::Scala => PackageManager::Sbt,
        Language::Unknown => PackageManager::Unknown,
    }
}

#[cfg(test)]
#[path = "package_manager.tests.rs"]
mod tests;
