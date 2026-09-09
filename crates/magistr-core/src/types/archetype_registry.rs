//! Archetype registry — bundled defaults for common language/framework combos.
//!
//! Provides sensible default commands, file patterns, and conventions for each
//! supported stack. Used when synthesizing archetypes from detected projects.

use std::path::PathBuf;

use super::archetype::Archetype;
use super::project_role::ProjectRole;
use super::quality_gate::{FormatConfig, QualityGate, TestingConfig};

/// A registry entry keyed by language and optional framework.
pub struct RegistryEntry {
    pub language: &'static str,
    pub framework: Option<&'static str>,
    pub quality_gate: QualityGate,
    pub format: FormatConfig,
    pub testing: TestingConfig,
    pub source_patterns: Vec<String>,
    pub naming_convention: &'static str,
}

/// Look up default archetype settings for a language/framework combo.
///
/// Returns `None` if the combo is not in the built-in registry.
#[must_use]
pub fn lookup(language: &str, framework: Option<&str>) -> Option<RegistryEntry> {
    // Try framework-specific match first, then language-only.
    if let Some(fw) = framework
        && let Some(entry) = lookup_entry(language, Some(fw))
    {
        return Some(entry);
    }
    lookup_entry(language, None)
}

/// Build a default [`Archetype`] from a registry lookup.
#[must_use]
pub fn default_archetype(
    id: &str,
    language: &str,
    framework: Option<&str>,
    root: &str,
    role: ProjectRole,
) -> Option<Archetype> {
    let entry = lookup(language, framework)?;
    Some(Archetype {
        id: id.into(),
        name: format_name(language, framework),
        language: entry.language.into(),
        framework: entry.framework.map(Into::into),
        root: PathBuf::from(root),
        role,
        quality_gate: entry.quality_gate,
        format: entry.format,
        testing: entry.testing,
        source_patterns: entry.source_patterns,
        test_patterns: vec![],
        config_patterns: vec![],
        naming_convention: Some(entry.naming_convention.into()),
        module_structure: None,
        model_preference: None,
    })
}

fn format_name(language: &str, framework: Option<&str>) -> String {
    match framework {
        Some(fw) => format!("{} ({})", capitalize(language), fw),
        None => capitalize(language),
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

fn lookup_entry(language: &str, framework: Option<&str>) -> Option<RegistryEntry> {
    match (language, framework) {
        // ── Rust ─────────────────────────────────────────────────────
        ("rust", _) => Some(RegistryEntry {
            language: "rust",
            framework: None,
            quality_gate: QualityGate {
                check: "cargo check --workspace --all-targets".into(),
                test: "cargo nextest run --workspace".into(),
                lint: "cargo clippy --workspace --all-targets -- -D warnings".into(),
            },
            format: FormatConfig {
                extensions: vec!["rs".into()],
                command: "cargo fmt".into(),
                exclusions: vec!["target".into()],
            },
            testing: TestingConfig {
                companion_pattern: "{name}.tests.{ext}".into(),
                test_dir: "alongside".into(),
            },
            source_patterns: vec!["src/**/*.rs".into()],
            naming_convention: "snake_case",
        }),

        // ── TypeScript / Next.js ─────────────────────────────────────
        ("typescript", Some("nextjs" | "next")) => Some(RegistryEntry {
            language: "typescript",
            framework: Some("nextjs"),
            quality_gate: QualityGate {
                check: "npx tsc --noEmit".into(),
                test: "npm test".into(),
                lint: "npm run lint".into(),
            },
            format: FormatConfig {
                extensions: vec!["ts".into(), "tsx".into(), "js".into(), "jsx".into()],
                command: "npx prettier --write .".into(),
                exclusions: vec!["node_modules".into(), ".next".into(), "dist".into()],
            },
            testing: TestingConfig {
                companion_pattern: "{name}.test.{ext}".into(),
                test_dir: "alongside".into(),
            },
            source_patterns: vec![
                "src/**/*.tsx".into(),
                "src/**/*.ts".into(),
                "app/**/*.tsx".into(),
            ],
            naming_convention: "camelCase",
        }),

        // ── TypeScript / React (plain) ───────────────────────────────
        ("typescript", Some("react")) => Some(RegistryEntry {
            language: "typescript",
            framework: Some("react"),
            quality_gate: QualityGate {
                check: "npx tsc --noEmit".into(),
                test: "npm test".into(),
                lint: "npx eslint .".into(),
            },
            format: FormatConfig {
                extensions: vec!["ts".into(), "tsx".into(), "js".into(), "jsx".into()],
                command: "npx prettier --write .".into(),
                exclusions: vec!["node_modules".into(), "build".into(), "dist".into()],
            },
            testing: TestingConfig {
                companion_pattern: "{name}.test.{ext}".into(),
                test_dir: "alongside".into(),
            },
            source_patterns: vec!["src/**/*.tsx".into(), "src/**/*.ts".into()],
            naming_convention: "camelCase",
        }),

        // ── TypeScript / Vue ─────────────────────────────────────────
        ("typescript" | "javascript", Some("vue" | "nuxt")) => Some(RegistryEntry {
            language: "typescript",
            framework: Some("vue"),
            quality_gate: QualityGate {
                check: "npx vue-tsc --noEmit".into(),
                test: "npm test".into(),
                lint: "npx eslint .".into(),
            },
            format: FormatConfig {
                extensions: vec!["ts".into(), "vue".into(), "js".into()],
                command: "npx prettier --write .".into(),
                exclusions: vec!["node_modules".into(), "dist".into(), ".nuxt".into()],
            },
            testing: TestingConfig {
                companion_pattern: "{name}.test.{ext}".into(),
                test_dir: "alongside".into(),
            },
            source_patterns: vec!["src/**/*.vue".into(), "src/**/*.ts".into()],
            naming_convention: "camelCase",
        }),

        // ── TypeScript / Svelte ──────────────────────────────────────
        ("typescript" | "javascript", Some("svelte" | "sveltekit")) => Some(RegistryEntry {
            language: "typescript",
            framework: Some("svelte"),
            quality_gate: QualityGate {
                check: "npx svelte-check".into(),
                test: "npm test".into(),
                lint: "npx eslint .".into(),
            },
            format: FormatConfig {
                extensions: vec!["ts".into(), "svelte".into(), "js".into()],
                command: "npx prettier --write .".into(),
                exclusions: vec!["node_modules".into(), ".svelte-kit".into(), "build".into()],
            },
            testing: TestingConfig {
                companion_pattern: "{name}.test.{ext}".into(),
                test_dir: "alongside".into(),
            },
            source_patterns: vec!["src/**/*.svelte".into(), "src/**/*.ts".into()],
            naming_convention: "camelCase",
        }),

        // ── TypeScript / Angular ─────────────────────────────────────
        ("typescript", Some("angular")) => Some(RegistryEntry {
            language: "typescript",
            framework: Some("angular"),
            quality_gate: QualityGate {
                check: "npx ng build --configuration production".into(),
                test: "npx ng test --watch=false".into(),
                lint: "npx ng lint".into(),
            },
            format: FormatConfig {
                extensions: vec!["ts".into(), "html".into(), "css".into()],
                command: "npx prettier --write .".into(),
                exclusions: vec!["node_modules".into(), "dist".into()],
            },
            testing: TestingConfig {
                companion_pattern: "{name}.spec.{ext}".into(),
                test_dir: "alongside".into(),
            },
            source_patterns: vec!["src/**/*.ts".into(), "src/**/*.html".into()],
            naming_convention: "camelCase",
        }),

        // ── TypeScript / Express ─────────────────────────────────────
        ("typescript" | "javascript", Some("express")) => Some(RegistryEntry {
            language: "typescript",
            framework: Some("express"),
            quality_gate: QualityGate {
                check: "npx tsc --noEmit".into(),
                test: "npm test".into(),
                lint: "npx eslint .".into(),
            },
            format: FormatConfig {
                extensions: vec!["ts".into(), "js".into()],
                command: "npx prettier --write .".into(),
                exclusions: vec!["node_modules".into(), "dist".into()],
            },
            testing: TestingConfig {
                companion_pattern: "{name}.test.{ext}".into(),
                test_dir: "alongside".into(),
            },
            source_patterns: vec!["src/**/*.ts".into()],
            naming_convention: "camelCase",
        }),

        // ── TypeScript (generic) ─────────────────────────────────────
        ("typescript", _) => Some(RegistryEntry {
            language: "typescript",
            framework: None,
            quality_gate: QualityGate {
                check: "npx tsc --noEmit".into(),
                test: "npm test".into(),
                lint: "npx eslint .".into(),
            },
            format: FormatConfig {
                extensions: vec!["ts".into(), "tsx".into(), "js".into(), "jsx".into()],
                command: "npx prettier --write .".into(),
                exclusions: vec!["node_modules".into(), "dist".into()],
            },
            testing: TestingConfig {
                companion_pattern: "{name}.test.{ext}".into(),
                test_dir: "alongside".into(),
            },
            source_patterns: vec!["src/**/*.ts".into()],
            naming_convention: "camelCase",
        }),

        // ── Python / Django ──────────────────────────────────────────
        ("python", Some("django")) => Some(RegistryEntry {
            language: "python",
            framework: Some("django"),
            quality_gate: QualityGate {
                check: "python manage.py check".into(),
                test: "python manage.py test".into(),
                lint: "ruff check .".into(),
            },
            format: FormatConfig {
                extensions: vec!["py".into()],
                command: "ruff format .".into(),
                exclusions: vec![".venv".into(), "__pycache__".into(), "migrations".into()],
            },
            testing: TestingConfig {
                companion_pattern: "test_{name}.{ext}".into(),
                test_dir: "alongside".into(),
            },
            source_patterns: vec!["**/*.py".into()],
            naming_convention: "snake_case",
        }),

        // ── Python / Flask ───────────────────────────────────────────
        ("python", Some("flask")) => Some(RegistryEntry {
            language: "python",
            framework: Some("flask"),
            quality_gate: QualityGate {
                check: "python -m py_compile app.py".into(),
                test: "pytest".into(),
                lint: "ruff check .".into(),
            },
            format: FormatConfig {
                extensions: vec!["py".into()],
                command: "ruff format .".into(),
                exclusions: vec![".venv".into(), "__pycache__".into()],
            },
            testing: TestingConfig {
                companion_pattern: "test_{name}.{ext}".into(),
                test_dir: "tests".into(),
            },
            source_patterns: vec!["**/*.py".into()],
            naming_convention: "snake_case",
        }),

        // ── Python (generic) ─────────────────────────────────────────
        ("python", _) => Some(RegistryEntry {
            language: "python",
            framework: None,
            quality_gate: QualityGate {
                check: "python -m py_compile .".into(),
                test: "pytest".into(),
                lint: "ruff check .".into(),
            },
            format: FormatConfig {
                extensions: vec!["py".into()],
                command: "ruff format .".into(),
                exclusions: vec![".venv".into(), "__pycache__".into()],
            },
            testing: TestingConfig {
                companion_pattern: "test_{name}.{ext}".into(),
                test_dir: "tests".into(),
            },
            source_patterns: vec!["**/*.py".into()],
            naming_convention: "snake_case",
        }),

        // ── Go ───────────────────────────────────────────────────────
        ("go", _) => Some(RegistryEntry {
            language: "go",
            framework: None,
            quality_gate: QualityGate {
                check: "go build ./...".into(),
                test: "go test ./...".into(),
                lint: "golangci-lint run".into(),
            },
            format: FormatConfig {
                extensions: vec!["go".into()],
                command: "gofmt -w .".into(),
                exclusions: vec!["vendor".into()],
            },
            testing: TestingConfig {
                companion_pattern: "{name}_test.{ext}".into(),
                test_dir: "alongside".into(),
            },
            source_patterns: vec!["**/*.go".into()],
            naming_convention: "camelCase",
        }),

        // ── Ruby / Rails ─────────────────────────────────────────────
        ("ruby", Some("rails")) => Some(RegistryEntry {
            language: "ruby",
            framework: Some("rails"),
            quality_gate: QualityGate {
                check: "bundle exec rails runner 'puts :ok'".into(),
                test: "bundle exec rspec".into(),
                lint: "bundle exec rubocop".into(),
            },
            format: FormatConfig {
                extensions: vec!["rb".into()],
                command: "bundle exec rubocop -A".into(),
                exclusions: vec!["vendor".into(), "tmp".into(), "log".into()],
            },
            testing: TestingConfig {
                companion_pattern: "{name}_spec.{ext}".into(),
                test_dir: "spec".into(),
            },
            source_patterns: vec!["app/**/*.rb".into(), "lib/**/*.rb".into()],
            naming_convention: "snake_case",
        }),

        // ── Ruby (generic) ──────────────────────────────────────────
        ("ruby", _) => Some(RegistryEntry {
            language: "ruby",
            framework: None,
            quality_gate: QualityGate {
                check: "ruby -c lib/**/*.rb".into(),
                test: "bundle exec rspec".into(),
                lint: "bundle exec rubocop".into(),
            },
            format: FormatConfig {
                extensions: vec!["rb".into()],
                command: "bundle exec rubocop -A".into(),
                exclusions: vec!["vendor".into()],
            },
            testing: TestingConfig {
                companion_pattern: "{name}_spec.{ext}".into(),
                test_dir: "spec".into(),
            },
            source_patterns: vec!["lib/**/*.rb".into()],
            naming_convention: "snake_case",
        }),

        // ── Java / Spring Boot ───────────────────────────────────────
        ("java", Some("spring" | "spring-boot" | "springboot")) => Some(RegistryEntry {
            language: "java",
            framework: Some("spring"),
            quality_gate: QualityGate {
                check: "./mvnw compile".into(),
                test: "./mvnw test".into(),
                lint: "./mvnw checkstyle:check".into(),
            },
            format: FormatConfig {
                extensions: vec!["java".into()],
                command: "./mvnw spotless:apply".into(),
                exclusions: vec!["target".into(), ".mvn".into()],
            },
            testing: TestingConfig {
                companion_pattern: "{name}Test.{ext}".into(),
                test_dir: "src/test/java".into(),
            },
            source_patterns: vec!["src/main/java/**/*.java".into()],
            naming_convention: "camelCase",
        }),

        // ── Java (generic) ──────────────────────────────────────────
        ("java", _) => Some(RegistryEntry {
            language: "java",
            framework: None,
            quality_gate: QualityGate {
                check: "./mvnw compile".into(),
                test: "./mvnw test".into(),
                lint: "./mvnw checkstyle:check".into(),
            },
            format: FormatConfig {
                extensions: vec!["java".into()],
                command: "./mvnw spotless:apply".into(),
                exclusions: vec!["target".into()],
            },
            testing: TestingConfig {
                companion_pattern: "{name}Test.{ext}".into(),
                test_dir: "src/test/java".into(),
            },
            source_patterns: vec!["src/**/*.java".into()],
            naming_convention: "camelCase",
        }),

        // ── C# / ASP.NET ────────────────────────────────────────────
        ("csharp", Some("aspnet" | "asp.net")) => Some(RegistryEntry {
            language: "csharp",
            framework: Some("aspnet"),
            quality_gate: QualityGate {
                check: "dotnet build".into(),
                test: "dotnet test".into(),
                lint: "dotnet format --verify-no-changes".into(),
            },
            format: FormatConfig {
                extensions: vec!["cs".into()],
                command: "dotnet format".into(),
                exclusions: vec!["bin".into(), "obj".into()],
            },
            testing: TestingConfig {
                companion_pattern: "{name}Tests.{ext}".into(),
                test_dir: "tests".into(),
            },
            source_patterns: vec!["**/*.cs".into()],
            naming_convention: "PascalCase",
        }),

        // ── C# (generic) ───────────────────────────────────────────
        ("csharp", _) => Some(RegistryEntry {
            language: "csharp",
            framework: None,
            quality_gate: QualityGate {
                check: "dotnet build".into(),
                test: "dotnet test".into(),
                lint: "dotnet format --verify-no-changes".into(),
            },
            format: FormatConfig {
                extensions: vec!["cs".into()],
                command: "dotnet format".into(),
                exclusions: vec!["bin".into(), "obj".into()],
            },
            testing: TestingConfig {
                companion_pattern: "{name}Tests.{ext}".into(),
                test_dir: "tests".into(),
            },
            source_patterns: vec!["**/*.cs".into()],
            naming_convention: "PascalCase",
        }),

        // ── PHP / Laravel ───────────────────────────────────────────
        ("php", Some("laravel")) => Some(RegistryEntry {
            language: "php",
            framework: Some("laravel"),
            quality_gate: QualityGate {
                check: "php artisan --version".into(),
                test: "php artisan test".into(),
                lint: "./vendor/bin/phpstan analyse".into(),
            },
            format: FormatConfig {
                extensions: vec!["php".into()],
                command: "./vendor/bin/pint".into(),
                exclusions: vec!["vendor".into(), "storage".into(), "bootstrap/cache".into()],
            },
            testing: TestingConfig {
                companion_pattern: "{name}Test.{ext}".into(),
                test_dir: "tests".into(),
            },
            source_patterns: vec!["app/**/*.php".into(), "routes/**/*.php".into()],
            naming_convention: "camelCase",
        }),

        // ── PHP (generic) ──────────────────────────────────────────
        ("php", _) => Some(RegistryEntry {
            language: "php",
            framework: None,
            quality_gate: QualityGate {
                check: "php -l src/".into(),
                test: "./vendor/bin/phpunit".into(),
                lint: "./vendor/bin/phpstan analyse".into(),
            },
            format: FormatConfig {
                extensions: vec!["php".into()],
                command: "./vendor/bin/php-cs-fixer fix".into(),
                exclusions: vec!["vendor".into()],
            },
            testing: TestingConfig {
                companion_pattern: "{name}Test.{ext}".into(),
                test_dir: "tests".into(),
            },
            source_patterns: vec!["src/**/*.php".into()],
            naming_convention: "camelCase",
        }),

        // ── Elixir / Phoenix ────────────────────────────────────────
        ("elixir", Some("phoenix")) => Some(RegistryEntry {
            language: "elixir",
            framework: Some("phoenix"),
            quality_gate: QualityGate {
                check: "mix compile --warnings-as-errors".into(),
                test: "mix test".into(),
                lint: "mix credo --strict".into(),
            },
            format: FormatConfig {
                extensions: vec!["ex".into(), "exs".into(), "heex".into()],
                command: "mix format".into(),
                exclusions: vec!["_build".into(), "deps".into()],
            },
            testing: TestingConfig {
                companion_pattern: "{name}_test.{ext}".into(),
                test_dir: "test".into(),
            },
            source_patterns: vec!["lib/**/*.ex".into(), "lib/**/*.heex".into()],
            naming_convention: "snake_case",
        }),

        // ── Elixir (generic) ───────────────────────────────────────
        ("elixir", _) => Some(RegistryEntry {
            language: "elixir",
            framework: None,
            quality_gate: QualityGate {
                check: "mix compile --warnings-as-errors".into(),
                test: "mix test".into(),
                lint: "mix credo --strict".into(),
            },
            format: FormatConfig {
                extensions: vec!["ex".into(), "exs".into()],
                command: "mix format".into(),
                exclusions: vec!["_build".into(), "deps".into()],
            },
            testing: TestingConfig {
                companion_pattern: "{name}_test.{ext}".into(),
                test_dir: "test".into(),
            },
            source_patterns: vec!["lib/**/*.ex".into()],
            naming_convention: "snake_case",
        }),

        _ => None,
    }
}

/// All languages supported by the registry.
pub const SUPPORTED_LANGUAGES: &[&str] = &[
    "rust",
    "typescript",
    "python",
    "go",
    "ruby",
    "java",
    "csharp",
    "php",
    "elixir",
];

/// All framework-specific entries in the registry.
pub const SUPPORTED_FRAMEWORKS: &[(&str, &str)] = &[
    ("typescript", "nextjs"),
    ("typescript", "react"),
    ("typescript", "vue"),
    ("typescript", "svelte"),
    ("typescript", "angular"),
    ("typescript", "express"),
    ("python", "django"),
    ("python", "flask"),
    ("ruby", "rails"),
    ("java", "spring"),
    ("csharp", "aspnet"),
    ("php", "laravel"),
    ("elixir", "phoenix"),
];

#[cfg(test)]
#[path = "archetype_registry.tests.rs"]
mod tests;
