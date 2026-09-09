use super::*;
use crate::detection::detect::detection::Detection;

/// Build a SmartStack with all high-confidence Rust defaults.
fn rust_smart_stack() -> SmartStack {
    SmartStack {
        project_name: "test-project".to_string(),
        language: Detection::new(Language::Rust, Confidence::high(), "Cargo.toml found"),
        package_manager: Detection::new(
            PackageManager::Cargo,
            Confidence::high(),
            "Cargo.toml found",
        ),
        framework: Detection::new(Framework::None, Confidence::new(80), "no framework"),
        test_framework: Detection::new(TestFramework::CargoTest, Confidence::high(), "built-in"),
        linter: Detection::new(Linter::Clippy, Confidence::high(), "built-in clippy"),
        formatter: Detection::new(Formatter::Rustfmt, Confidence::high(), "built-in rustfmt"),
        workspace: Detection::new(Workspace::None, Confidence::new(80), "no workspace"),
        ci_provider: Detection::new(CiProvider::None, Confidence::low(), "no CI"),
        framework_detail: None,
    }
}

fn nextjs_smart_stack() -> SmartStack {
    SmartStack {
        project_name: "my-app".to_string(),
        language: Detection::new(
            Language::TypeScript,
            Confidence::high(),
            "tsconfig.json found",
        ),
        package_manager: Detection::new(
            PackageManager::Npm,
            Confidence::high(),
            "package-lock.json found",
        ),
        framework: Detection::new(Framework::Next, Confidence::high(), "next.config.js found"),
        test_framework: Detection::new(
            TestFramework::Jest,
            Confidence::medium(),
            "jest in devDependencies",
        ),
        linter: Detection::new(Linter::Eslint, Confidence::high(), "ESLint config found"),
        formatter: Detection::new(
            Formatter::Prettier,
            Confidence::medium(),
            "defaulting to Prettier",
        ),
        workspace: Detection::new(Workspace::None, Confidence::new(80), "no workspace"),
        ci_provider: Detection::new(CiProvider::None, Confidence::low(), "no CI"),
        framework_detail: Some("App Router".to_string()),
    }
}

fn low_confidence_stack() -> SmartStack {
    SmartStack {
        project_name: "unknown".to_string(),
        language: Detection::none(Language::Unknown),
        package_manager: Detection::none(PackageManager::Unknown),
        framework: Detection::none(Framework::None),
        test_framework: Detection::none(TestFramework::Unknown),
        linter: Detection::new(Linter::None, Confidence::low(), "no linter"),
        formatter: Detection::new(Formatter::None, Confidence::low(), "no formatter"),
        workspace: Detection::new(Workspace::None, Confidence::low(), "no workspace"),
        ci_provider: Detection::new(CiProvider::None, Confidence::low(), "no CI"),
        framework_detail: None,
    }
}

// ── detect_common_setup ────────────────────────────────────────

#[test]
fn detects_rust_cli_archetype() {
    let smart = rust_smart_stack();
    let setup = detect_common_setup(&smart).unwrap();
    assert_eq!(setup.name, "Rust CLI");
    assert!(setup.confidence.is_high());
}

#[test]
fn detects_nextjs_app_archetype() {
    let smart = nextjs_smart_stack();
    let setup = detect_common_setup(&smart).unwrap();
    assert_eq!(setup.name, "Next.js App");
    assert!(setup.description.contains("App Router"));
}

#[test]
fn detects_nextjs_pages_archetype() {
    let mut smart = nextjs_smart_stack();
    smart.framework_detail = Some("Pages Router".to_string());
    let setup = detect_common_setup(&smart).unwrap();
    assert_eq!(setup.name, "Next.js App (Pages Router)");
}

#[test]
fn detects_django_rest_archetype() {
    let smart = SmartStack {
        project_name: "api".to_string(),
        language: Detection::new(Language::Python, Confidence::high(), "pyproject.toml"),
        package_manager: Detection::new(PackageManager::Uv, Confidence::high(), "uv.lock"),
        framework: Detection::new(Framework::Django, Confidence::high(), "manage.py found"),
        test_framework: Detection::new(TestFramework::Pytest, Confidence::high(), "conftest.py"),
        linter: Detection::new(Linter::Ruff, Confidence::new(85), "ruff config"),
        formatter: Detection::new(Formatter::Ruff, Confidence::new(85), "ruff formatter"),
        workspace: Detection::new(Workspace::None, Confidence::new(80), "none"),
        ci_provider: Detection::new(CiProvider::None, Confidence::low(), "no CI"),
        framework_detail: Some("REST Framework".to_string()),
    };
    let setup = detect_common_setup(&smart).unwrap();
    assert_eq!(setup.name, "Django REST API");
}

#[test]
fn detects_fastapi_archetype() {
    let smart = SmartStack {
        project_name: "service".to_string(),
        language: Detection::new(Language::Python, Confidence::high(), "pyproject.toml"),
        package_manager: Detection::new(PackageManager::Uv, Confidence::high(), "uv.lock"),
        framework: Detection::new(Framework::FastApi, Confidence::medium(), "fastapi in deps"),
        test_framework: Detection::new(TestFramework::Pytest, Confidence::high(), "conftest.py"),
        linter: Detection::new(Linter::Ruff, Confidence::new(85), "ruff config"),
        formatter: Detection::new(Formatter::Ruff, Confidence::new(85), "ruff formatter"),
        workspace: Detection::new(Workspace::None, Confidence::new(80), "none"),
        ci_provider: Detection::new(CiProvider::None, Confidence::low(), "no CI"),
        framework_detail: None,
    };
    let setup = detect_common_setup(&smart).unwrap();
    assert_eq!(setup.name, "FastAPI Service");
}

#[test]
fn detects_rust_axum_archetype() {
    let mut smart = rust_smart_stack();
    smart.framework = Detection::new(Framework::Axum, Confidence::new(85), "axum in Cargo.toml");
    let setup = detect_common_setup(&smart).unwrap();
    assert_eq!(setup.name, "Rust Web (Axum)");
}

#[test]
fn detects_go_service_archetype() {
    let smart = SmartStack {
        project_name: "svc".to_string(),
        language: Detection::new(Language::Go, Confidence::high(), "go.mod found"),
        package_manager: Detection::new(PackageManager::Go, Confidence::high(), "go.mod"),
        framework: Detection::new(Framework::None, Confidence::new(80), "no framework"),
        test_framework: Detection::new(
            TestFramework::GoTest,
            Confidence::high(),
            "built-in go test",
        ),
        linter: Detection::new(
            Linter::GolangciLint,
            Confidence::high(),
            "golangci-lint config",
        ),
        formatter: Detection::new(Formatter::Gofmt, Confidence::high(), "built-in gofmt"),
        workspace: Detection::new(Workspace::None, Confidence::new(80), "none"),
        ci_provider: Detection::new(CiProvider::None, Confidence::low(), "no CI"),
        framework_detail: None,
    };
    let setup = detect_common_setup(&smart).unwrap();
    assert_eq!(setup.name, "Go Service");
}

#[test]
fn detects_go_gin_archetype() {
    let smart = SmartStack {
        project_name: "web".to_string(),
        language: Detection::new(Language::Go, Confidence::high(), "go.mod found"),
        package_manager: Detection::new(PackageManager::Go, Confidence::high(), "go.mod"),
        framework: Detection::new(Framework::Gin, Confidence::medium(), "gin in go.mod"),
        test_framework: Detection::new(
            TestFramework::GoTest,
            Confidence::high(),
            "built-in go test",
        ),
        linter: Detection::new(
            Linter::GolangciLint,
            Confidence::high(),
            "golangci-lint config",
        ),
        formatter: Detection::new(Formatter::Gofmt, Confidence::high(), "built-in gofmt"),
        workspace: Detection::new(Workspace::None, Confidence::new(80), "none"),
        ci_provider: Detection::new(CiProvider::None, Confidence::low(), "no CI"),
        framework_detail: None,
    };
    let setup = detect_common_setup(&smart).unwrap();
    assert_eq!(setup.name, "Go Web (Gin)");
}

#[test]
fn detects_rails_archetype() {
    let smart = SmartStack {
        project_name: "blog".to_string(),
        language: Detection::new(Language::Ruby, Confidence::high(), "Gemfile found"),
        package_manager: Detection::new(PackageManager::Bundler, Confidence::high(), "Gemfile"),
        framework: Detection::new(
            Framework::Rails,
            Confidence::high(),
            "config/routes.rb found",
        ),
        test_framework: Detection::new(TestFramework::Rspec, Confidence::high(), ".rspec"),
        linter: Detection::new(Linter::Rubocop, Confidence::high(), ".rubocop.yml found"),
        formatter: Detection::new(
            Formatter::Rubocop,
            Confidence::medium(),
            "RuboCop formatter",
        ),
        workspace: Detection::new(Workspace::None, Confidence::new(80), "none"),
        ci_provider: Detection::new(CiProvider::None, Confidence::low(), "no CI"),
        framework_detail: None,
    };
    let setup = detect_common_setup(&smart).unwrap();
    assert_eq!(setup.name, "Rails App");
}

#[test]
fn detects_spring_boot_archetype() {
    let smart = SmartStack {
        project_name: "api".to_string(),
        language: Detection::new(Language::Java, Confidence::high(), "pom.xml found"),
        package_manager: Detection::new(PackageManager::Maven, Confidence::high(), "pom.xml"),
        framework: Detection::new(Framework::Spring, Confidence::high(), "Spring config found"),
        test_framework: Detection::new(TestFramework::Junit, Confidence::new(80), "JVM default"),
        linter: Detection::new(Linter::None, Confidence::low(), "none"),
        formatter: Detection::new(Formatter::None, Confidence::low(), "none"),
        workspace: Detection::new(Workspace::None, Confidence::new(80), "none"),
        ci_provider: Detection::new(CiProvider::None, Confidence::low(), "no CI"),
        framework_detail: None,
    };
    let setup = detect_common_setup(&smart).unwrap();
    assert_eq!(setup.name, "Spring Boot");
}

#[test]
fn detects_spring_kotlin_archetype() {
    let smart = SmartStack {
        project_name: "api".to_string(),
        language: Detection::new(
            Language::Kotlin,
            Confidence::high(),
            "build.gradle.kts found",
        ),
        package_manager: Detection::new(
            PackageManager::Gradle,
            Confidence::high(),
            "build.gradle.kts",
        ),
        framework: Detection::new(Framework::Spring, Confidence::high(), "Spring config found"),
        test_framework: Detection::new(TestFramework::Junit, Confidence::new(80), "JVM default"),
        linter: Detection::new(Linter::Ktlint, Confidence::low(), "defaulting to ktlint"),
        formatter: Detection::new(Formatter::Ktfmt, Confidence::low(), "defaulting to ktfmt"),
        workspace: Detection::new(Workspace::None, Confidence::new(80), "none"),
        ci_provider: Detection::new(CiProvider::None, Confidence::low(), "no CI"),
        framework_detail: None,
    };
    let setup = detect_common_setup(&smart).unwrap();
    assert_eq!(setup.name, "Spring Boot (Kotlin)");
}

#[test]
fn detects_express_archetype() {
    let smart = SmartStack {
        project_name: "api".to_string(),
        language: Detection::new(Language::TypeScript, Confidence::high(), "tsconfig.json"),
        package_manager: Detection::new(
            PackageManager::Npm,
            Confidence::high(),
            "package-lock.json",
        ),
        framework: Detection::new(Framework::Express, Confidence::medium(), "express in deps"),
        test_framework: Detection::new(
            TestFramework::Jest,
            Confidence::medium(),
            "jest in devDeps",
        ),
        linter: Detection::new(Linter::Eslint, Confidence::high(), "ESLint config"),
        formatter: Detection::new(
            Formatter::Prettier,
            Confidence::medium(),
            "defaulting to Prettier",
        ),
        workspace: Detection::new(Workspace::None, Confidence::new(80), "none"),
        ci_provider: Detection::new(CiProvider::None, Confidence::low(), "no CI"),
        framework_detail: None,
    };
    let setup = detect_common_setup(&smart).unwrap();
    assert_eq!(setup.name, "Express API");
}

#[test]
fn no_archetype_for_unknown_project() {
    let smart = low_confidence_stack();
    assert!(detect_common_setup(&smart).is_none());
}

// ── skippable_categories ───────────────────────────────────────

#[test]
fn rust_project_skips_most_categories() {
    let smart = rust_smart_stack();
    let skippable = skippable_categories(&smart);
    assert!(skippable.contains(&"language"));
    assert!(skippable.contains(&"package_manager"));
    assert!(skippable.contains(&"test_framework"));
    assert!(skippable.contains(&"linter"));
    assert!(skippable.contains(&"formatter"));
    // framework and workspace are 80, not high enough to skip
    assert!(!skippable.contains(&"framework"));
    assert!(!skippable.contains(&"workspace"));
}

#[test]
fn low_confidence_skips_nothing() {
    let smart = low_confidence_stack();
    let skippable = skippable_categories(&smart);
    assert!(skippable.is_empty());
}

#[test]
fn nextjs_skips_language_pm_framework_linter() {
    let smart = nextjs_smart_stack();
    let skippable = skippable_categories(&smart);
    assert!(skippable.contains(&"language"));
    assert!(skippable.contains(&"package_manager"));
    assert!(skippable.contains(&"framework"));
    assert!(skippable.contains(&"linter"));
    // test_framework (medium) and formatter (medium) need review
    assert!(!skippable.contains(&"test_framework"));
    assert!(!skippable.contains(&"formatter"));
}

// ── wizard_summary ─────────────────────────────────────────────

#[test]
fn wizard_summary_splits_confirmed_and_review() {
    let smart = nextjs_smart_stack();
    let summary = wizard_summary(&smart);

    // High-confidence categories → confirmed
    let confirmed_cats: Vec<&str> = summary.confirmed.iter().map(|c| c.category).collect();
    assert!(confirmed_cats.contains(&"language"));
    assert!(confirmed_cats.contains(&"framework"));

    // Medium-confidence → needs review
    let review_cats: Vec<&str> = summary.needs_review.iter().map(|r| r.category).collect();
    assert!(review_cats.contains(&"test_framework"));
    assert!(review_cats.contains(&"formatter"));
}

#[test]
fn wizard_summary_includes_common_setup() {
    let smart = nextjs_smart_stack();
    let summary = wizard_summary(&smart);
    assert!(summary.common_setup.is_some());
    assert_eq!(summary.common_setup.unwrap().name, "Next.js App");
}

#[test]
fn wizard_summary_no_setup_for_unknown() {
    let smart = low_confidence_stack();
    let summary = wizard_summary(&smart);
    assert!(summary.common_setup.is_none());
    assert!(summary.confirmed.is_empty());
    assert!(!summary.needs_review.is_empty());
}

#[test]
fn wizard_summary_framework_value_includes_detail() {
    let smart = nextjs_smart_stack();
    let summary = wizard_summary(&smart);
    let fw = summary
        .confirmed
        .iter()
        .find(|c| c.category == "framework")
        .unwrap();
    assert_eq!(fw.value, "next (App Router)");
}

#[test]
fn wizard_summary_all_confirmed_for_rust() {
    let smart = rust_smart_stack();
    let summary = wizard_summary(&smart);
    // language, package_manager, test_framework, linter, formatter → confirmed (5)
    // framework (80), workspace (80), ci_provider (low) → needs_review (3)
    assert_eq!(summary.confirmed.len(), 5);
    assert_eq!(summary.needs_review.len(), 3);
}
