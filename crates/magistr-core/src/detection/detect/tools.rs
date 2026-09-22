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
        Language::Python
            if manifest.has_dep("pytest")
                || manifest.has_python_tool("pytest")
                || has_file(dir, &["pytest.ini", "conftest.py"]) =>
        {
            TestFramework::Pytest
        }
        Language::Go => TestFramework::GoTest,
        Language::Java | Language::Kotlin if manifest.has_dep("junit") => TestFramework::Junit,
        Language::Ruby => {
            if dir.join(".rspec").exists() || manifest.has_dep("rspec") {
                TestFramework::Rspec
            } else if manifest.has_dep("minitest") {
                TestFramework::Minitest
            } else {
                TestFramework::Unknown
            }
        }
        Language::Elixir => TestFramework::Exunit,
        Language::Swift => TestFramework::Xctest,
        Language::Cpp if has_file(dir, &["CTestConfig.cmake", "CTestTestfile.cmake"]) => {
            TestFramework::Ctest
        }
        Language::CSharp if manifest.has_dep("xunit") => TestFramework::Xunit,
        Language::Dart => TestFramework::Unknown, // dart test (built-in)
        Language::Php => TestFramework::Unknown,  // PHPUnit
        _ => TestFramework::Unknown,
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
    if manifest.has_dep("jest") || manifest.js_tools.contains("jest") {
        return TestFramework::Jest;
    }
    if manifest.has_dep("mocha") {
        return TestFramework::Mocha;
    }

    TestFramework::Unknown
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
            } else if manifest.has_dep("eslint")
                || manifest.js_tools.contains("eslintConfig")
                || has_file(
                    dir,
                    &[
                        "eslint.config.js",
                        "eslint.config.mjs",
                        "eslint.config.cjs",
                        "eslint.config.ts",
                        ".eslintrc",
                        ".eslintrc.json",
                        ".eslintrc.js",
                        ".eslintrc.cjs",
                        ".eslintrc.yml",
                        ".eslintrc.yaml",
                    ],
                )
            {
                Linter::Eslint
            } else {
                Linter::None
            }
        }
        Language::Python => {
            if has_file(dir, &["ruff.toml", ".ruff.toml"])
                || manifest.has_python_tool("ruff")
                || manifest.has_dep("ruff")
            {
                return Linter::Ruff;
            }
            if manifest.has_python_tool("flake8") || manifest.has_dep("flake8") {
                return Linter::Flake8;
            }
            Linter::None
        }
        Language::Go
            if has_file(
                dir,
                &[
                    ".golangci.yml",
                    ".golangci.yaml",
                    ".golangci.toml",
                    ".golangci.json",
                ],
            ) =>
        {
            Linter::GolangciLint
        }
        Language::Ruby if manifest.has_dep("rubocop") || dir.join(".rubocop.yml").is_file() => {
            Linter::Rubocop
        }
        Language::Elixir if manifest.has_dep("credo") || dir.join(".credo.exs").is_file() => {
            Linter::Credo
        }
        Language::Swift if dir.join(".swiftlint.yml").is_file() => Linter::Swiftlint,
        Language::Kotlin if manifest.has_dep("ktlint") => Linter::Ktlint,
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
            } else if manifest.has_dep("prettier")
                || manifest.js_tools.contains("prettier")
                || has_file(
                    dir,
                    &[
                        ".prettierrc",
                        ".prettierrc.json",
                        ".prettierrc.yml",
                        ".prettierrc.yaml",
                        ".prettierrc.js",
                        ".prettierrc.cjs",
                        "prettier.config.js",
                        "prettier.config.cjs",
                        "prettier.config.mjs",
                    ],
                )
            {
                Formatter::Prettier
            } else {
                Formatter::None
            }
        }
        Language::Python => {
            if has_file(dir, &["ruff.toml", ".ruff.toml"])
                || manifest.has_python_tool("ruff")
                || manifest.has_dep("ruff")
            {
                return Formatter::Ruff;
            }
            if manifest.has_dep("black") || manifest.has_python_tool("black") {
                Formatter::Black
            } else {
                Formatter::None
            }
        }
        Language::Go => Formatter::Gofmt,
        Language::Ruby if manifest.has_dep("rubocop") || dir.join(".rubocop.yml").is_file() => {
            Formatter::Rubocop
        }
        Language::Elixir => Formatter::MixFormat,
        Language::Swift if dir.join(".swiftformat").is_file() => Formatter::Swiftformat,
        Language::Kotlin if manifest.has_dep("ktfmt") => Formatter::Ktfmt,
        Language::CSharp => Formatter::DotnetFormat,
        _ => Formatter::None,
    }
}

fn has_file(dir: &Path, names: &[&str]) -> bool {
    names.iter().any(|name| dir.join(name).is_file())
}

#[cfg(test)]
#[path = "tools.tests.rs"]
mod tests;
