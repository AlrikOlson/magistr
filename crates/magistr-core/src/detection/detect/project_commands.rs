//! Derive project commands from observed manifests and tool configuration.
//! Preset proposals are replaced before explicitly selected layers are applied.
use std::path::Path;

use super::{config_parser::parse_manifests, tools};
use crate::types::{Formatter, Language, Linter, PackageManager, Preset, TestFramework};

pub fn configure_project_commands(
    preset: &mut Preset,
    dir: &Path,
    lang: Language,
    manager: PackageManager,
) {
    preset.quality_gate.check.clear();
    preset.quality_gate.test.clear();
    preset.quality_gate.lint.clear();
    preset.format.command.clear();
    let manifest = parse_manifests(dir);
    match lang {
        Language::Rust if dir.join("Cargo.toml").is_file() => {
            preset.quality_gate.check = "cargo check --all-targets".into();
            preset.quality_gate.test = if dir.join(".config/nextest.toml").is_file() {
                "cargo nextest run"
            } else {
                "cargo test"
            }
            .into();
            preset.quality_gate.lint = "cargo clippy --all-targets -- -D warnings".into();
            preset.format.command = "cargo fmt".into();
        }
        Language::TypeScript | Language::JavaScript => {
            configure_javascript(preset, dir, manager, &manifest);
        }
        Language::Python => {
            if manifest.has_dep("mypy") || manifest.has_python_tool("mypy") {
                preset.quality_gate.check = "python -m mypy .".into();
            }
            if tools::detect_test_framework(dir, lang, &manifest) == TestFramework::Pytest {
                preset.quality_gate.test = "python -m pytest".into();
            }
            preset.quality_gate.lint = match tools::detect_linter(dir, lang, &manifest) {
                Linter::Ruff => "python -m ruff check .",
                Linter::Flake8 => "python -m flake8 .",
                _ => "",
            }
            .into();
            preset.format.command = match tools::detect_formatter(dir, lang, &manifest) {
                Formatter::Ruff => "python -m ruff format {file}",
                Formatter::Black => "python -m black {file}",
                _ => "",
            }
            .into();
        }
        Language::Go if dir.join("go.mod").is_file() => {
            preset.quality_gate.check = "go build ./...".into();
            preset.quality_gate.test = "go test ./...".into();
            preset.quality_gate.lint = "go vet ./...".into();
            preset.format.command = "gofmt -w {file}".into();
        }
        Language::Elixir if dir.join("mix.exs").is_file() => {
            preset.quality_gate.check = "mix compile --warnings-as-errors".into();
            preset.quality_gate.test = "mix test".into();
            preset.format.command = "mix format {file}".into();
            if manifest.has_dep("credo") {
                preset.quality_gate.lint = "mix credo --strict".into();
            }
        }
        _ => {}
    }
}

fn configure_javascript(
    preset: &mut Preset,
    dir: &Path,
    manager: PackageManager,
    manifest: &super::config_parser::ManifestData,
) {
    let script = |names: &[&str]| -> Option<String> {
        let runner = match manager {
            PackageManager::Npm => "npm",
            PackageManager::Pnpm => "pnpm",
            PackageManager::Yarn => "yarn",
            PackageManager::Bun => "bun",
            _ => {
                return names
                    .iter()
                    .any(|name| manifest.has_script(name))
                    .then(String::new);
            }
        };
        names
            .iter()
            .find(|name| manifest.has_script(name))
            .map(|name| format!("{runner} run {name}"))
    };
    // Local executables never download undeclared tooling during a gate.
    let bin = |command: &str| format!("./node_modules/.bin/{command}");
    preset.quality_gate.check = script(&["check", "typecheck", "type-check"])
        .or_else(|| {
            (manifest.has_dep("typescript") && dir.join("tsconfig.json").is_file())
                .then(|| bin("tsc --noEmit"))
        })
        .unwrap_or_default();
    preset.quality_gate.test = script(&["test"]).unwrap_or_else(|| {
        match tools::detect_test_framework(dir, Language::JavaScript, manifest) {
            TestFramework::Vitest => bin("vitest run"),
            TestFramework::Jest => bin("jest"),
            TestFramework::Mocha => bin("mocha"),
            _ => String::new(),
        }
    });
    preset.quality_gate.lint = script(&["lint"]).unwrap_or_else(|| {
        match tools::detect_linter(dir, Language::JavaScript, manifest) {
            Linter::Eslint => bin("eslint ."),
            Linter::Biome => bin("biome check ."),
            _ => String::new(),
        }
    });
    preset.format.command = script(&["format"]).unwrap_or_else(|| {
        match tools::detect_formatter(dir, Language::JavaScript, manifest) {
            Formatter::Prettier => bin("prettier --write {file}"),
            Formatter::Biome => bin("biome check --write ."),
            _ => String::new(),
        }
    });
}

#[cfg(test)]
#[path = "project_commands.tests.rs"]
mod tests;
