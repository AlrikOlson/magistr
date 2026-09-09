use std::path::Path;

use super::ci::detect_ci_provider;
use super::confidence::Confidence;
use super::config_parser::{ManifestData, parse_manifests};
use super::detection::Detection;
use super::framework::detect_framework;
use super::language::detect_language;
use super::package_manager::detect_package_manager;
use super::project_name::detect_project_name;
use super::scoring::{Scorable, ScoringContext};
use super::tools::{detect_formatter, detect_linter, detect_test_framework};
use super::workspace::detect_workspace;
use crate::error::{MagistrError, Result};
use crate::types::*;

/// Detect the full project stack with per-category confidence scores.
///
/// This is the enhanced version of `detect_stack` used by `--smart` mode.
/// It runs the same detection logic, then scores confidence based on signal strength.
pub fn detect_stack_smart(dir: &Path) -> Result<SmartStack> {
    let dir = dir.canonicalize().map_err(|e| MagistrError::io(dir, e))?;

    if !dir.is_dir() {
        return Err(MagistrError::DirNotFound(dir));
    }

    let manifest = parse_manifests(&dir);

    let language = detect_language(&dir);
    let package_manager = detect_package_manager(&dir, language);
    let framework = detect_framework(&dir, language, &manifest);
    let test_framework = detect_test_framework(&dir, language, &manifest);
    let linter = detect_linter(&dir, language, &manifest);
    let formatter = detect_formatter(&dir, language, &manifest);
    let project_name = detect_project_name(&dir, language);
    let workspace = detect_workspace(&dir);
    let ci_provider = detect_ci_provider(&dir);

    let framework_detail = detect_framework_detail(&dir, framework, &manifest);

    let scoring_ctx = ScoringContext {
        dir: &dir,
        language,
        manifest: &manifest,
    };

    Ok(SmartStack {
        project_name,
        language: language.score(&scoring_ctx),
        package_manager: score_package_manager(&dir, language, package_manager),
        framework: score_framework(&dir, framework, &manifest),
        test_framework: score_test_framework(&dir, language, test_framework, &manifest),
        linter: score_linter(&dir, language, linter, &manifest),
        formatter: score_formatter(language, formatter),
        workspace: score_workspace(&dir, workspace),
        ci_provider: score_ci_provider(&dir, ci_provider),
        framework_detail,
    })
}

// ---------------------------------------------------------------------------
// Confidence scoring functions
// ---------------------------------------------------------------------------

#[allow(dead_code)] // Will be used when language scoring is wired in
fn score_language(dir: &Path, lang: Language) -> Detection<Language> {
    match lang {
        Language::Unknown => Detection::none(lang),
        Language::Rust => Detection::new(lang, Confidence::high(), "Cargo.toml found"),
        Language::TypeScript => {
            if dir.join("tsconfig.json").exists() {
                Detection::new(lang, Confidence::high(), "tsconfig.json found")
            } else {
                Detection::new(
                    lang,
                    Confidence::medium(),
                    "typescript in package.json dependencies",
                )
            }
        }
        Language::JavaScript => Detection::new(
            lang,
            Confidence::new(85),
            "package.json found without TypeScript",
        ),
        Language::Python => {
            let mut evidence = Vec::new();
            if dir.join("pyproject.toml").exists() {
                evidence.push("pyproject.toml found".to_string());
            }
            if dir.join("setup.py").exists() {
                evidence.push("setup.py found".to_string());
            }
            if dir.join("setup.cfg").exists() {
                evidence.push("setup.cfg found".to_string());
            }
            let confidence = if evidence.len() >= 2 {
                Confidence::high()
            } else {
                Confidence::new(90)
            };
            Detection::with_evidence(lang, confidence, evidence)
        }
        Language::Go => Detection::new(lang, Confidence::high(), "go.mod found"),
        Language::Java => {
            if dir.join("pom.xml").exists() {
                Detection::new(lang, Confidence::high(), "pom.xml found")
            } else {
                Detection::new(lang, Confidence::new(85), "build.gradle found")
            }
        }
        Language::Kotlin => {
            if dir.join("build.gradle.kts").exists() {
                Detection::new(lang, Confidence::high(), "build.gradle.kts found")
            } else {
                Detection::new(
                    lang,
                    Confidence::medium(),
                    "kotlin references in build.gradle",
                )
            }
        }
        Language::Ruby => Detection::new(lang, Confidence::high(), "Gemfile found"),
        Language::Elixir => Detection::new(lang, Confidence::high(), "mix.exs found"),
        Language::Swift => Detection::new(lang, Confidence::high(), "Package.swift found"),
        Language::Cpp => {
            if dir.join("CMakeLists.txt").exists() {
                Detection::new(lang, Confidence::new(90), "CMakeLists.txt found")
            } else {
                Detection::new(lang, Confidence::low(), "C++ references in Makefile")
            }
        }
        Language::CSharp => Detection::new(lang, Confidence::new(90), ".csproj or .sln file found"),
        Language::Dart => Detection::new(lang, Confidence::high(), "pubspec.yaml found"),
        Language::Php => Detection::new(lang, Confidence::new(85), "composer.json found"),
        Language::Scala => Detection::new(lang, Confidence::high(), "build.sbt found"),
    }
}

fn score_package_manager(
    dir: &Path,
    lang: Language,
    pm: PackageManager,
) -> Detection<PackageManager> {
    match pm {
        PackageManager::Unknown => Detection::none(pm),
        PackageManager::Cargo => Detection::new(pm, Confidence::high(), "Cargo.toml found"),
        PackageManager::Pnpm => Detection::new(pm, Confidence::high(), "pnpm-lock.yaml found"),
        PackageManager::Yarn => Detection::new(pm, Confidence::high(), "yarn.lock found"),
        PackageManager::Bun => Detection::new(pm, Confidence::high(), "bun.lockb found"),
        PackageManager::Npm => {
            if dir.join("package-lock.json").exists() {
                Detection::new(pm, Confidence::high(), "package-lock.json found")
            } else {
                Detection::new(pm, Confidence::medium(), "package.json found, assuming npm")
            }
        }
        PackageManager::Uv => Detection::new(pm, Confidence::high(), "uv.lock found"),
        PackageManager::Poetry => Detection::new(pm, Confidence::high(), "poetry.lock found"),
        PackageManager::Pipenv => Detection::new(pm, Confidence::high(), "Pipfile found"),
        PackageManager::Pip => {
            Detection::new(pm, Confidence::low(), "Python project, defaulting to pip")
        }
        PackageManager::Go => Detection::new(pm, Confidence::high(), "go.mod found"),
        PackageManager::Maven => Detection::new(pm, Confidence::high(), "pom.xml found"),
        PackageManager::Gradle => {
            Detection::new(pm, Confidence::high(), "build.gradle(.kts) found")
        }
        PackageManager::Bundler => Detection::new(pm, Confidence::high(), "Gemfile found"),
        PackageManager::Mix => Detection::new(pm, Confidence::high(), "mix.exs found"),
        PackageManager::Spm => Detection::new(pm, Confidence::high(), "Package.swift found"),
        PackageManager::Cmake => Detection::new(pm, Confidence::new(85), "CMakeLists.txt found"),
        PackageManager::Meson => Detection::new(pm, Confidence::new(85), "meson.build found"),
        PackageManager::Make => {
            // Make is a fallback for C/C++, lower confidence
            let conf = if lang == Language::Cpp {
                Confidence::low()
            } else {
                Confidence::new(60)
            };
            Detection::new(pm, conf, "Makefile found")
        }
        PackageManager::Dotnet => Detection::new(pm, Confidence::new(90), ".csproj found"),
        PackageManager::Pub => Detection::new(pm, Confidence::high(), "pubspec.yaml found"),
        PackageManager::Composer => Detection::new(pm, Confidence::new(85), "composer.json found"),
        PackageManager::Sbt => Detection::new(pm, Confidence::high(), "build.sbt found"),
    }
}

fn score_framework(dir: &Path, fw: Framework, manifest: &ManifestData) -> Detection<Framework> {
    match fw {
        Framework::None => Detection::new(fw, Confidence::new(80), "no framework detected"),
        // Config-file-based detection → high confidence
        Framework::Next => {
            let has_config = ["next.config.js", "next.config.ts", "next.config.mjs"]
                .iter()
                .any(|f| dir.join(f).exists());
            if has_config {
                Detection::new(fw, Confidence::high(), "next.config.* found")
            } else {
                Detection::new(
                    fw,
                    Confidence::medium(),
                    "next in package.json dependencies",
                )
            }
        }
        Framework::Nuxt => {
            let has_config = ["nuxt.config.ts", "nuxt.config.js"]
                .iter()
                .any(|f| dir.join(f).exists());
            if has_config {
                Detection::new(fw, Confidence::high(), "nuxt.config.* found")
            } else {
                Detection::new(fw, Confidence::medium(), "nuxt in dependencies")
            }
        }
        Framework::SvelteKit => Detection::new(fw, Confidence::high(), "svelte.config.js found"),
        Framework::Astro => Detection::new(fw, Confidence::high(), "astro.config.* found"),
        Framework::Remix => Detection::new(fw, Confidence::high(), "remix.config.* found"),
        Framework::Vite => Detection::new(fw, Confidence::new(85), "vite.config.* found"),
        Framework::Angular => Detection::new(fw, Confidence::high(), "angular.json found"),
        // Dependency-only detection → medium confidence
        Framework::Express | Framework::Fastify | Framework::Hono => Detection::new(
            fw,
            Confidence::medium(),
            format!("{} in package.json dependencies", fw.as_str()),
        ),
        Framework::React | Framework::Vue => Detection::new(
            fw,
            Confidence::medium(),
            format!("{} in package.json dependencies", fw.as_str()),
        ),
        Framework::Django => {
            if dir.join("manage.py").exists() {
                Detection::new(fw, Confidence::high(), "manage.py found")
            } else {
                Detection::new(fw, Confidence::medium(), "django in dependencies")
            }
        }
        Framework::FastApi => Detection::new(fw, Confidence::medium(), "fastapi in dependencies"),
        Framework::Flask => Detection::new(fw, Confidence::medium(), "flask in dependencies"),
        Framework::Rails => {
            if dir.join("config").join("routes.rb").exists() {
                Detection::new(fw, Confidence::high(), "config/routes.rb found")
            } else {
                Detection::new(fw, Confidence::new(85), "rails in Gemfile")
            }
        }
        Framework::Sinatra => Detection::new(fw, Confidence::medium(), "sinatra in Gemfile"),
        Framework::Phoenix => Detection::new(fw, Confidence::medium(), "phoenix in mix.exs"),
        Framework::Axum
        | Framework::Actix
        | Framework::Rocket
        | Framework::Leptos
        | Framework::Dioxus
        | Framework::Bevy => {
            if manifest.has_dep(fw.as_str()) {
                Detection::new(
                    fw,
                    Confidence::new(85),
                    format!("{} in Cargo.toml dependencies", fw.as_str()),
                )
            } else {
                Detection::new(
                    fw,
                    Confidence::medium(),
                    format!("{} detected", fw.as_str()),
                )
            }
        }
        Framework::Tauri => {
            if dir.join("tauri.conf.json").exists() || dir.join("src-tauri").exists() {
                Detection::new(fw, Confidence::high(), "tauri config found")
            } else {
                Detection::new(fw, Confidence::new(80), "tauri in Cargo.toml dependencies")
            }
        }
        Framework::Gin | Framework::Echo | Framework::Fiber | Framework::Chi => Detection::new(
            fw,
            Confidence::medium(),
            format!("{} in go.mod dependencies", fw.as_str()),
        ),
        Framework::Spring => {
            if dir
                .join("src/main/resources/application.properties")
                .exists()
                || dir.join("src/main/resources/application.yml").exists()
            {
                Detection::new(fw, Confidence::high(), "Spring config files found")
            } else {
                Detection::new(fw, Confidence::new(80), "spring in build config")
            }
        }
        Framework::Quarkus => Detection::new(fw, Confidence::new(80), "quarkus in pom.xml"),
        Framework::Ktor => Detection::new(fw, Confidence::medium(), "ktor in build.gradle.kts"),
        Framework::Vapor => Detection::new(fw, Confidence::medium(), "vapor in Package.swift"),
        Framework::AspNet => Detection::new(fw, Confidence::new(85), "ASP.NET markers found"),
        Framework::Blazor => Detection::new(fw, Confidence::new(85), "Blazor markers in .csproj"),
        Framework::Maui => Detection::new(fw, Confidence::new(85), "MAUI markers in .csproj"),
        Framework::Flutter => {
            if dir.join("pubspec.yaml").exists() {
                Detection::new(fw, Confidence::high(), "Flutter SDK in pubspec.yaml")
            } else {
                Detection::new(fw, Confidence::medium(), "Flutter detected")
            }
        }
        Framework::ReactNative => Detection::new(
            fw,
            Confidence::medium(),
            "react-native in package.json dependencies",
        ),
        Framework::Micronaut => {
            Detection::new(fw, Confidence::new(80), "micronaut in build config")
        }
        Framework::NestJS => Detection::new(
            fw,
            Confidence::medium(),
            "@nestjs/core in package.json dependencies",
        ),
        Framework::Laravel => {
            if dir.join("artisan").exists() {
                Detection::new(fw, Confidence::high(), "artisan CLI found")
            } else {
                Detection::new(
                    fw,
                    Confidence::medium(),
                    "laravel/framework in composer.json",
                )
            }
        }
    }
}

fn score_test_framework(
    dir: &Path,
    _lang: Language,
    tf: TestFramework,
    manifest: &ManifestData,
) -> Detection<TestFramework> {
    match tf {
        TestFramework::Unknown => Detection::none(tf),
        // Built-in test frameworks → high confidence
        TestFramework::CargoTest => Detection::new(
            tf,
            Confidence::high(),
            "Rust project uses built-in cargo test",
        ),
        TestFramework::GoTest => {
            Detection::new(tf, Confidence::high(), "Go project uses built-in go test")
        }
        TestFramework::Exunit => Detection::new(
            tf,
            Confidence::high(),
            "Elixir project uses built-in ExUnit",
        ),
        TestFramework::Xctest => {
            Detection::new(tf, Confidence::high(), "Swift project uses built-in XCTest")
        }
        // Config-file-based → high confidence
        TestFramework::Vitest => {
            let has_config =
                dir.join("vitest.config.ts").exists() || dir.join("vitest.config.js").exists();
            if has_config {
                Detection::new(tf, Confidence::high(), "vitest.config.* found")
            } else {
                Detection::new(tf, Confidence::medium(), "vitest in devDependencies")
            }
        }
        TestFramework::Jest => {
            let has_config =
                dir.join("jest.config.js").exists() || dir.join("jest.config.ts").exists();
            if has_config {
                Detection::new(tf, Confidence::high(), "jest.config.* found")
            } else {
                Detection::new(tf, Confidence::medium(), "jest in devDependencies")
            }
        }
        TestFramework::Pytest => {
            if dir.join("pytest.ini").exists() || dir.join("conftest.py").exists() {
                Detection::new(tf, Confidence::high(), "pytest config found")
            } else if manifest.has_dep("pytest") {
                Detection::new(tf, Confidence::new(85), "pytest in dependencies")
            } else {
                // Python defaults to pytest
                Detection::new(
                    tf,
                    Confidence::low(),
                    "Python project, defaulting to pytest",
                )
            }
        }
        TestFramework::Mocha => {
            Detection::new(tf, Confidence::medium(), "mocha in devDependencies")
        }
        TestFramework::Junit => {
            Detection::new(tf, Confidence::new(80), "JVM project, defaulting to JUnit")
        }
        TestFramework::Rspec => {
            if dir.join(".rspec").exists() {
                Detection::new(tf, Confidence::high(), ".rspec config found")
            } else {
                Detection::new(tf, Confidence::medium(), "rspec in Gemfile")
            }
        }
        TestFramework::Minitest => Detection::new(tf, Confidence::medium(), "minitest detected"),
        TestFramework::Ctest => {
            Detection::new(tf, Confidence::low(), "C++ project, defaulting to CTest")
        }
        TestFramework::Xunit => {
            Detection::new(tf, Confidence::new(80), "C# project, defaulting to xUnit")
        }
    }
}

fn score_linter(
    dir: &Path,
    _lang: Language,
    linter: Linter,
    manifest: &ManifestData,
) -> Detection<Linter> {
    match linter {
        Linter::None => Detection::new(linter, Confidence::low(), "no linter detected"),
        Linter::Clippy => Detection::new(
            linter,
            Confidence::high(),
            "Rust project uses built-in clippy",
        ),
        Linter::GolangciLint => {
            if dir.join(".golangci.yml").exists() || dir.join(".golangci.yaml").exists() {
                Detection::new(linter, Confidence::high(), "golangci-lint config found")
            } else {
                Detection::new(
                    linter,
                    Confidence::low(),
                    "Go project, defaulting to golangci-lint",
                )
            }
        }
        Linter::Eslint => {
            let has_config = [
                ".eslintrc.js",
                ".eslintrc.json",
                ".eslintrc.yml",
                "eslint.config.js",
                "eslint.config.mjs",
            ]
            .iter()
            .any(|f| dir.join(f).exists());
            if has_config {
                Detection::new(linter, Confidence::high(), "ESLint config found")
            } else if manifest.has_dep("eslint") {
                Detection::new(linter, Confidence::medium(), "eslint in devDependencies")
            } else {
                Detection::new(
                    linter,
                    Confidence::low(),
                    "JS/TS project, defaulting to ESLint",
                )
            }
        }
        Linter::Biome => {
            if dir.join("biome.json").exists() || dir.join("biome.jsonc").exists() {
                Detection::new(linter, Confidence::high(), "biome config found")
            } else {
                Detection::new(linter, Confidence::medium(), "biome in devDependencies")
            }
        }
        Linter::Ruff => {
            Detection::new(linter, Confidence::new(85), "ruff config in pyproject.toml")
        }
        Linter::Flake8 => {
            if dir.join(".flake8").exists() {
                Detection::new(linter, Confidence::high(), ".flake8 config found")
            } else {
                Detection::new(linter, Confidence::medium(), "flake8 in dependencies")
            }
        }
        Linter::Rubocop => {
            if dir.join(".rubocop.yml").exists() {
                Detection::new(linter, Confidence::high(), ".rubocop.yml found")
            } else {
                Detection::new(
                    linter,
                    Confidence::low(),
                    "Ruby project, defaulting to RuboCop",
                )
            }
        }
        Linter::Credo => Detection::new(linter, Confidence::medium(), "credo in mix.exs"),
        Linter::Swiftlint => {
            if dir.join(".swiftlint.yml").exists() {
                Detection::new(linter, Confidence::high(), ".swiftlint.yml found")
            } else {
                Detection::new(
                    linter,
                    Confidence::low(),
                    "Swift project, defaulting to SwiftLint",
                )
            }
        }
        Linter::Ktlint => Detection::new(
            linter,
            Confidence::low(),
            "Kotlin project, defaulting to ktlint",
        ),
    }
}

fn score_formatter(lang: Language, fmt: Formatter) -> Detection<Formatter> {
    match fmt {
        Formatter::None => Detection::new(fmt, Confidence::low(), "no formatter detected"),
        // Built-in / standard formatters → high confidence
        Formatter::Rustfmt => Detection::new(
            fmt,
            Confidence::high(),
            "Rust project uses built-in rustfmt",
        ),
        Formatter::Gofmt => {
            Detection::new(fmt, Confidence::high(), "Go project uses built-in gofmt")
        }
        Formatter::MixFormat => Detection::new(
            fmt,
            Confidence::high(),
            "Elixir project uses built-in mix format",
        ),
        // Language defaults → medium confidence
        Formatter::Prettier => Detection::new(
            fmt,
            Confidence::medium(),
            "JS/TS project, defaulting to Prettier",
        ),
        Formatter::Black => Detection::new(
            fmt,
            Confidence::medium(),
            "Python project, defaulting to Black",
        ),
        Formatter::Ruff => Detection::new(fmt, Confidence::new(85), "ruff formatter config found"),
        Formatter::Biome => Detection::new(fmt, Confidence::new(85), "biome used as formatter"),
        Formatter::Rubocop => Detection::new(
            fmt,
            Confidence::medium(),
            "Ruby project, using RuboCop formatter",
        ),
        Formatter::Swiftformat => Detection::new(
            fmt,
            Confidence::low(),
            format!("{} project, defaulting to swiftformat", lang),
        ),
        Formatter::Ktfmt => Detection::new(
            fmt,
            Confidence::low(),
            format!("{} project, defaulting to ktfmt", lang),
        ),
        Formatter::DotnetFormat => {
            Detection::new(fmt, Confidence::new(80), "C# project, using dotnet format")
        }
    }
}

fn score_ci_provider(dir: &Path, ci: CiProvider) -> Detection<CiProvider> {
    match ci {
        CiProvider::None => Detection::new(ci, Confidence::low(), "no CI/CD config detected"),
        CiProvider::GitHubActions => {
            // Count workflow files for confidence boost
            let workflow_count = dir
                .join(".github/workflows")
                .read_dir()
                .ok()
                .map(|entries| {
                    entries
                        .filter_map(|e| e.ok())
                        .filter(|e| {
                            let name = e.file_name();
                            let name = name.to_string_lossy();
                            name.ends_with(".yml") || name.ends_with(".yaml")
                        })
                        .count()
                })
                .unwrap_or(0);
            if workflow_count > 1 {
                Detection::new(
                    ci,
                    Confidence::high(),
                    format!("{workflow_count} GitHub Actions workflows found"),
                )
            } else {
                Detection::new(ci, Confidence::high(), "GitHub Actions workflow found")
            }
        }
        CiProvider::GitLabCi => Detection::new(ci, Confidence::high(), ".gitlab-ci.yml found"),
        CiProvider::CircleCi => {
            Detection::new(ci, Confidence::high(), ".circleci/config.yml found")
        }
        CiProvider::TravisCi => Detection::new(ci, Confidence::high(), ".travis.yml found"),
    }
}

fn score_workspace(dir: &Path, ws: Workspace) -> Detection<Workspace> {
    match ws {
        Workspace::None => {
            Detection::new(ws, Confidence::new(80), "no workspace/monorepo detected")
        }
        Workspace::Cargo => Detection::new(ws, Confidence::high(), "[workspace] in Cargo.toml"),
        Workspace::Turborepo => Detection::new(ws, Confidence::high(), "turbo.json found"),
        Workspace::Nx => Detection::new(ws, Confidence::high(), "nx.json found"),
        Workspace::Pnpm => Detection::new(ws, Confidence::high(), "pnpm-workspace.yaml found"),
        Workspace::Npm | Workspace::Yarn => {
            if dir.join("yarn.lock").exists() {
                Detection::new(
                    ws,
                    Confidence::new(90),
                    "workspaces in package.json + yarn.lock",
                )
            } else {
                Detection::new(ws, Confidence::new(85), "workspaces field in package.json")
            }
        }
        Workspace::Lerna => Detection::new(ws, Confidence::high(), "lerna.json found"),
        Workspace::Rush => Detection::new(ws, Confidence::high(), "rush.json found"),
        Workspace::DotnetSln => Detection::new(ws, Confidence::new(90), ".sln file found"),
        Workspace::Moon => Detection::new(ws, Confidence::high(), "moon.yml found"),
    }
}

// ---------------------------------------------------------------------------
// Deep framework detail detection
// ---------------------------------------------------------------------------

/// Detect additional framework detail beyond the base framework enum.
///
/// Returns a human-readable string like "App Router", "Pages Router",
/// "REST Framework", etc.
fn detect_framework_detail(dir: &Path, fw: Framework, manifest: &ManifestData) -> Option<String> {
    match fw {
        Framework::Next => detect_next_detail(dir),
        Framework::Django => detect_django_detail(manifest),
        _ => None,
    }
}

/// Next.js: App Router vs Pages Router vs hybrid.
fn detect_next_detail(dir: &Path) -> Option<String> {
    let has_app_dir = dir.join("app").is_dir() || dir.join("src/app").is_dir();
    let has_pages_dir = dir.join("pages").is_dir() || dir.join("src/pages").is_dir();

    match (has_app_dir, has_pages_dir) {
        (true, true) => Some("Hybrid (App Router + Pages)".to_string()),
        (true, false) => Some("App Router".to_string()),
        (false, true) => Some("Pages Router".to_string()),
        (false, false) => None,
    }
}

/// Django: check for REST framework or channels.
fn detect_django_detail(manifest: &ManifestData) -> Option<String> {
    if manifest.has_dep("djangorestframework") || manifest.has_dep("django-rest-framework") {
        Some("REST Framework".to_string())
    } else if manifest.has_dep("channels") || manifest.has_dep("django-channels") {
        Some("Channels".to_string())
    } else {
        None
    }
}

#[cfg(test)]
#[path = "smart.tests.rs"]
mod tests;
