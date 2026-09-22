use std::path::Path;

use crate::types::{Language, PackageManager};

/// Detect the package manager based on language and lock files.
pub fn detect_package_manager(dir: &Path, lang: Language) -> PackageManager {
    match lang {
        Language::Rust => PackageManager::Cargo,
        Language::TypeScript | Language::JavaScript => detect_js_package_manager(dir),
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

fn detect_js_package_manager(dir: &Path) -> PackageManager {
    let manifest = super::config_parser::parse_manifests(dir);
    if let Some(declaration) = &manifest.package_manager {
        return match declaration.split('@').next().unwrap_or("") {
            "npm" => PackageManager::Npm,
            "pnpm" => PackageManager::Pnpm,
            "yarn" => PackageManager::Yarn,
            "bun" => PackageManager::Bun,
            _ => PackageManager::Unknown,
        };
    }
    for (file, manager) in [
        ("pnpm-lock.yaml", PackageManager::Pnpm),
        ("yarn.lock", PackageManager::Yarn),
        ("bun.lockb", PackageManager::Bun),
        ("bun.lock", PackageManager::Bun),
        ("package-lock.json", PackageManager::Npm),
        ("npm-shrinkwrap.json", PackageManager::Npm),
    ] {
        if dir.join(file).is_file() {
            return manager;
        }
    }
    if manifest.bun_engine
        || dir.join("bunfig.toml").is_file()
        || manifest.scripts.values().any(|command| {
            command
                .split("&&")
                .any(|part| part.split_whitespace().next() == Some("bun"))
        })
    {
        return PackageManager::Bun;
    }
    PackageManager::Unknown
}

#[cfg(test)]
#[path = "package_manager.tests.rs"]
mod tests;
