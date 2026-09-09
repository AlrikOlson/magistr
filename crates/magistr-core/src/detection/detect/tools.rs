use std::path::Path;

use super::config_parser::ManifestData;
use crate::types::{Formatter, Language, Linter, TestFramework};

/// Detect the test framework for a given language.
pub(crate) fn detect_test_framework(
    dir: &Path,
    lang: Language,
    manifest: &ManifestData,
) -> TestFramework {
    match lang {
        Language::Rust => TestFramework::CargoTest,
        Language::TypeScript | Language::JavaScript => detect_js_test_framework(dir, manifest),
        Language::Python => TestFramework::Pytest,
        Language::Go => TestFramework::GoTest,
        Language::Java | Language::Kotlin => TestFramework::Junit,
        Language::Ruby => {
            if dir.join(".rspec").exists() || manifest.has_dep("rspec") {
                TestFramework::Rspec
            } else {
                TestFramework::Minitest
            }
        }
        Language::Elixir => TestFramework::Exunit,
        Language::Swift => TestFramework::Xctest,
        Language::Cpp => TestFramework::Ctest,
        Language::CSharp => TestFramework::Xunit,
        Language::Dart => TestFramework::Unknown, // dart test (built-in)
        Language::Php => TestFramework::Unknown,  // PHPUnit
        Language::Scala => TestFramework::Junit,  // ScalaTest/MUnit (JVM)
        Language::Unknown => TestFramework::Unknown,
    }
}

fn detect_js_test_framework(dir: &Path, manifest: &ManifestData) -> TestFramework {
    // Check for vitest config files
    for ext in ["ts", "js", "mts", "mjs"] {
        if dir.join(format!("vitest.config.{ext}")).exists() {
            return TestFramework::Vitest;
        }
    }

    // Check for jest config files
    for ext in ["ts", "js", "cjs", "mjs"] {
        if dir.join(format!("jest.config.{ext}")).exists() {
            return TestFramework::Jest;
        }
    }

    // Check parsed dependencies for test framework
    if manifest.has_dep("vitest") {
        return TestFramework::Vitest;
    }
    if manifest.has_dep("jest") {
        return TestFramework::Jest;
    }
    if manifest.has_dep("mocha") {
        return TestFramework::Mocha;
    }

    // Default for new JS/TS projects
    TestFramework::Vitest
}

/// Check if a Biome config is present (biome.json, biome.jsonc, or @biomejs/biome dep).
fn has_biome_config(dir: &Path, manifest: &ManifestData) -> bool {
    dir.join("biome.json").exists()
        || dir.join("biome.jsonc").exists()
        || manifest.has_dep("@biomejs/biome")
}

/// Detect the linter for a given language.
pub(crate) fn detect_linter(dir: &Path, lang: Language, manifest: &ManifestData) -> Linter {
    match lang {
        Language::Rust => Linter::Clippy,
        Language::TypeScript | Language::JavaScript => {
            if has_biome_config(dir, manifest) {
                Linter::Biome
            } else {
                Linter::Eslint
            }
        }
        Language::Python => {
            if dir.join("ruff.toml").exists() || manifest.has_python_tool("ruff") {
                return Linter::Ruff;
            }
            if manifest.has_python_tool("flake8") || manifest.has_dep("flake8") {
                return Linter::Flake8;
            }
            Linter::Ruff
        }
        Language::Go => Linter::GolangciLint,
        Language::Ruby => Linter::Rubocop,
        Language::Elixir => Linter::Credo,
        Language::Swift => Linter::Swiftlint,
        Language::Kotlin => Linter::Ktlint,
        _ => Linter::None,
    }
}

/// Detect the formatter for a given language.
pub(crate) fn detect_formatter(dir: &Path, lang: Language, manifest: &ManifestData) -> Formatter {
    match lang {
        Language::Rust => Formatter::Rustfmt,
        Language::TypeScript | Language::JavaScript => {
            if has_biome_config(dir, manifest) {
                Formatter::Biome
            } else {
                Formatter::Prettier
            }
        }
        Language::Python => {
            if dir.join("ruff.toml").exists() || manifest.has_python_tool("ruff") {
                return Formatter::Ruff;
            }
            Formatter::Black
        }
        Language::Go => Formatter::Gofmt,
        Language::Ruby => Formatter::Rubocop,
        Language::Elixir => Formatter::MixFormat,
        Language::Swift => Formatter::Swiftformat,
        Language::Kotlin => Formatter::Ktfmt,
        Language::CSharp => Formatter::DotnetFormat,
        _ => Formatter::None,
    }
}

#[cfg(test)]
#[path = "tools.tests.rs"]
mod tests;
