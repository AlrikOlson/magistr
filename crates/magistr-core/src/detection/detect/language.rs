use std::path::Path;

use crate::types::Language;

/// Detect the primary language from manifest files in the project directory.
///
/// The primary is the first entry of [`detect_languages`] — one ladder, so the
/// two can never disagree about priority.
pub fn detect_language(dir: &Path) -> Language {
    detect_languages(dir)
        .into_iter()
        .next()
        .unwrap_or(Language::Unknown)
}

/// Detect every language evidenced by manifest files in the project directory,
/// in priority order (the first entry is the primary).
///
/// A directory carrying both `Cargo.toml` and `package.json` is a polyglot
/// project, not a Rust one — each evidenced stack gets its own entry so the
/// tree builder can emit one detected project per stack.
pub(crate) fn detect_languages(dir: &Path) -> Vec<Language> {
    let mut found = Vec::new();
    let push = |lang: Language, languages: &mut Vec<Language>| {
        if !languages.contains(&lang) {
            languages.push(lang);
        }
    };

    if dir.join("Cargo.toml").exists() {
        push(Language::Rust, &mut found);
    }

    if dir.join("package.json").exists() {
        let is_typescript = dir.join("tsconfig.json").exists()
            || std::fs::read_to_string(dir.join("package.json"))
                .is_ok_and(|contents| contents.contains("\"typescript\""));
        if is_typescript {
            push(Language::TypeScript, &mut found);
        } else {
            push(Language::JavaScript, &mut found);
        }
    }

    if dir.join("pyproject.toml").exists()
        || dir.join("setup.py").exists()
        || dir.join("setup.cfg").exists()
    {
        push(Language::Python, &mut found);
    }

    if dir.join("go.mod").exists() {
        push(Language::Go, &mut found);
    }

    if dir.join("pom.xml").exists() {
        push(Language::Java, &mut found);
    }

    if dir.join("build.gradle.kts").exists() {
        push(Language::Kotlin, &mut found);
    }

    if dir.join("build.gradle").exists() {
        if std::fs::read_to_string(dir.join("build.gradle"))
            .is_ok_and(|contents| contents.contains("kotlin"))
        {
            push(Language::Kotlin, &mut found);
        } else {
            push(Language::Java, &mut found);
        }
    }

    if dir.join("Gemfile").exists() {
        push(Language::Ruby, &mut found);
    }

    if dir.join("mix.exs").exists() {
        push(Language::Elixir, &mut found);
    }

    if dir.join("pubspec.yaml").exists() {
        push(Language::Dart, &mut found);
    }

    if dir.join("build.sbt").exists() {
        push(Language::Scala, &mut found);
    }

    if dir.join("Package.swift").exists() {
        push(Language::Swift, &mut found);
    }

    if dir.join("CMakeLists.txt").exists() {
        push(Language::Cpp, &mut found);
    }

    if dir.join("Makefile").exists()
        && std::fs::read_to_string(dir.join("Makefile")).is_ok_and(|contents| {
            contents.contains("g++") || contents.contains("clang++") || contents.contains("CXX")
        })
    {
        push(Language::Cpp, &mut found);
    }

    // Check for .csproj or .sln files
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.ends_with(".csproj") || name.ends_with(".sln") {
                push(Language::CSharp, &mut found);
                break;
            }
        }
    }

    if dir.join("composer.json").exists() {
        push(Language::Php, &mut found);
    }

    found
}

#[cfg(test)]
#[path = "language.tests.rs"]
mod tests;
