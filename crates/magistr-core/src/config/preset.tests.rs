use super::*;

use crate::config::fallback_preset::fallback_preset;
use crate::config::preset_framework::{apply_package_manager_overlay, load_preset_with_framework};
use crate::types::{Framework, PackageManager};

#[test]
fn loads_all_known_presets() {
    let languages = [
        Language::Rust,
        Language::TypeScript,
        Language::JavaScript,
        Language::Python,
        Language::Go,
        Language::Java,
        Language::Kotlin,
        Language::Ruby,
        Language::Elixir,
        Language::Swift,
        Language::Cpp,
        Language::CSharp,
        Language::Dart,
        Language::Php,
        Language::Scala,
    ];

    for lang in languages {
        let preset = load_preset(lang);
        assert!(
            preset.is_ok(),
            "failed to load preset for {lang}: {:?}",
            preset.err()
        );
        let preset = preset.unwrap();
        assert!(!preset.quality_gate.check.is_empty());
        assert!(!preset.quality_gate.test.is_empty());
        assert!(!preset.allowed_tools.is_empty());
    }
}

#[test]
fn unknown_language_returns_error() {
    assert!(load_preset(Language::Unknown).is_err());
}

#[test]
fn fallback_preset_has_defaults() {
    let preset = fallback_preset();
    assert!(!preset.allowed_tools.is_empty());
}

#[test]
fn nextjs_overlay_merges_onto_typescript() {
    let base = load_preset(Language::TypeScript).unwrap();
    let merged = load_preset_with_framework(Language::TypeScript, Framework::Next).unwrap();

    // Check command was overridden
    assert_ne!(base.quality_gate.check, merged.quality_gate.check);
    assert!(merged.quality_gate.check.contains("next lint"));

    // Format was not overridden — should match base
    assert_eq!(base.format.command, merged.format.command);
}

#[test]
fn django_overlay_merges_onto_python() {
    let merged = load_preset_with_framework(Language::Python, Framework::Django).unwrap();

    assert!(merged.quality_gate.check.contains("manage.py"));
    assert!(merged.quality_gate.test.contains("manage.py"));
    assert!(merged.allowed_tools.iter().any(|t| t.contains("manage.py")));
}

#[test]
fn axum_overlay_merges_onto_rust() {
    let merged = load_preset_with_framework(Language::Rust, Framework::Axum).unwrap();

    assert!(merged.quality_gate.check.contains("--all-targets"));
}

#[test]
fn no_overlay_returns_base_unchanged() {
    let base = load_preset(Language::TypeScript).unwrap();
    let merged = load_preset_with_framework(Language::TypeScript, Framework::React).unwrap();

    assert_eq!(base, merged);
}

#[test]
fn framework_none_returns_base_unchanged() {
    let base = load_preset(Language::Rust).unwrap();
    let merged = load_preset_with_framework(Language::Rust, Framework::None).unwrap();

    assert_eq!(base, merged);
}

#[test]
fn all_overlays_parse_successfully() {
    let frameworks_with_overlays = [
        // JS/TS
        (Language::TypeScript, Framework::Next),
        (Language::TypeScript, Framework::Vue),
        (Language::TypeScript, Framework::Nuxt),
        (Language::TypeScript, Framework::SvelteKit),
        (Language::TypeScript, Framework::Express),
        (Language::TypeScript, Framework::Fastify),
        (Language::TypeScript, Framework::Hono),
        // Python
        (Language::Python, Framework::Django),
        (Language::Python, Framework::FastApi),
        (Language::Python, Framework::Flask),
        // Rust
        (Language::Rust, Framework::Axum),
        (Language::Rust, Framework::Tauri),
        (Language::Rust, Framework::Bevy),
        (Language::Rust, Framework::Leptos),
        (Language::Rust, Framework::Dioxus),
        // Go
        (Language::Go, Framework::Gin),
        (Language::Go, Framework::Echo),
        (Language::Go, Framework::Fiber),
        (Language::Go, Framework::Chi),
        // Ruby
        (Language::Ruby, Framework::Rails),
        // Elixir
        (Language::Elixir, Framework::Phoenix),
        // Java/Kotlin
        (Language::Java, Framework::Spring),
        (Language::Java, Framework::Quarkus),
        (Language::Kotlin, Framework::Ktor),
        // Swift
        (Language::Swift, Framework::Vapor),
        // C#
        (Language::CSharp, Framework::AspNet),
        (Language::CSharp, Framework::Blazor),
        (Language::CSharp, Framework::Maui),
        // Dart
        (Language::Dart, Framework::Flutter),
        // JS/TS (additional)
        (Language::TypeScript, Framework::NestJS),
        (Language::TypeScript, Framework::ReactNative),
        // Java (additional)
        (Language::Java, Framework::Micronaut),
        // Kotlin (own overrides)
        (Language::Kotlin, Framework::Quarkus),
        // PHP
        (Language::Php, Framework::Laravel),
    ];

    for (lang, fw) in frameworks_with_overlays {
        let result = load_preset_with_framework(lang, fw);
        assert!(
            result.is_ok(),
            "failed to load preset with overlay for {lang}+{fw}: {:?}",
            result.err()
        );
    }
}

#[test]
fn rust_preset_uses_all_targets() {
    let preset = load_preset(Language::Rust).unwrap();

    assert!(preset.quality_gate.check.contains("--all-targets"));
    assert!(preset.quality_gate.lint.contains("--all-targets"));
}

#[test]
fn python_preset_mypy_excludes_venv() {
    let preset = load_preset(Language::Python).unwrap();

    assert!(preset.quality_gate.check.contains("--exclude"));
    assert!(preset.quality_gate.check.contains(".venv"));
}

#[test]
fn typescript_preset_prettier_ignore_path() {
    let preset = load_preset(Language::TypeScript).unwrap();

    assert!(preset.format.command.contains("--ignore-path .gitignore"));
    assert!(preset.format.extensions.contains(&"json".to_string()));
    assert!(preset.format.extensions.contains(&"css".to_string()));
    assert!(preset.format.extensions.contains(&"md".to_string()));
}

#[test]
fn ruby_preset_bundle_check() {
    let preset = load_preset(Language::Ruby).unwrap();

    assert_eq!(preset.quality_gate.check, "bundle check");
    assert!(preset.format.command.contains("bundle exec"));
}

#[test]
fn go_preset_lint_timeout() {
    let preset = load_preset(Language::Go).unwrap();

    assert!(preset.quality_gate.lint.contains("--timeout 5m"));
}

#[test]
fn swift_preset_swiftlint_strict() {
    let preset = load_preset(Language::Swift).unwrap();

    assert!(preset.quality_gate.lint.contains("--strict"));
}

#[test]
fn cpp_preset_portable_lint_and_parallel_build() {
    let preset = load_preset(Language::Cpp).unwrap();

    assert!(preset.quality_gate.lint.contains("find src"));
    assert!(preset.quality_gate.lint.contains("xargs clang-tidy"));
    assert!(!preset.quality_gate.lint.contains("src/**/*.cpp"));
    assert!(preset.quality_gate.check.contains("--parallel"));
}

#[test]
fn gradle_pm_overlay_merges_onto_java() {
    let base = load_preset(Language::Java).unwrap();
    let merged = apply_package_manager_overlay(base, PackageManager::Gradle).unwrap();

    assert!(merged.quality_gate.check.contains("gradlew"));
    assert!(merged.quality_gate.test.contains("gradlew"));
    assert!(merged.quality_gate.lint.contains("gradlew"));
}

#[test]
fn maven_pm_returns_base_unchanged() {
    let base = load_preset(Language::Java).unwrap();
    let merged = apply_package_manager_overlay(base.clone(), PackageManager::Maven).unwrap();

    assert_eq!(base, merged);
}

#[test]
fn django_overlay_excludes_migrations() {
    let merged = load_preset_with_framework(Language::Python, Framework::Django).unwrap();

    assert!(merged.quality_gate.check.contains("migrations"));
    assert!(merged.format.exclusions.contains(&"migrations".to_string()));
}

#[test]
fn nextjs_overlay_excludes_dot_next() {
    let merged = load_preset_with_framework(Language::TypeScript, Framework::Next).unwrap();

    assert!(merged.format.exclusions.contains(&".next".to_string()));
}

#[test]
fn vue_overlay_uses_vue_tsc() {
    let merged = load_preset_with_framework(Language::TypeScript, Framework::Vue).unwrap();

    assert!(merged.quality_gate.check.contains("vue-tsc"));
}

#[test]
fn sveltekit_overlay_uses_svelte_check() {
    let merged = load_preset_with_framework(Language::TypeScript, Framework::SvelteKit).unwrap();

    assert!(merged.quality_gate.check.contains("svelte-check"));
    assert!(
        merged
            .format
            .exclusions
            .contains(&".svelte-kit".to_string())
    );
}

#[test]
fn gin_overlay_uses_race_detector() {
    let merged = load_preset_with_framework(Language::Go, Framework::Gin).unwrap();

    assert!(merged.quality_gate.test.contains("-race"));
    assert!(merged.quality_gate.check.contains("go vet"));
}

#[test]
fn phoenix_overlay_excludes_build_dirs() {
    let merged = load_preset_with_framework(Language::Elixir, Framework::Phoenix).unwrap();

    assert!(merged.format.exclusions.contains(&"_build".to_string()));
    assert!(merged.format.exclusions.contains(&"deps".to_string()));
}

#[test]
fn vapor_overlay_excludes_dot_build() {
    let merged = load_preset_with_framework(Language::Swift, Framework::Vapor).unwrap();

    assert!(merged.format.exclusions.contains(&".build".to_string()));
}

#[test]
fn aspnet_overlay_uses_warnaserror() {
    let merged = load_preset_with_framework(Language::CSharp, Framework::AspNet).unwrap();

    assert!(merged.quality_gate.check.contains("--warnaserror"));
}

#[test]
fn tauri_overlay_excludes_node_modules() {
    let merged = load_preset_with_framework(Language::Rust, Framework::Tauri).unwrap();

    assert!(
        merged
            .format
            .exclusions
            .contains(&"node_modules".to_string())
    );
    assert!(merged.allowed_tools.iter().any(|t| t.contains("tauri")));
}

#[test]
fn flask_overlay_uses_pytest() {
    let merged = load_preset_with_framework(Language::Python, Framework::Flask).unwrap();

    assert!(merged.quality_gate.test.contains("pytest"));
    assert!(merged.allowed_tools.iter().any(|t| t.contains("flask")));
}

// --- New language preset tests ---

#[test]
fn dart_preset_uses_dart_analyze() {
    let preset = load_preset(Language::Dart).unwrap();

    assert!(preset.quality_gate.check.contains("dart analyze"));
    assert!(preset.quality_gate.test.contains("dart test"));
    assert!(preset.format.extensions.contains(&"dart".to_string()));
}

#[test]
fn php_preset_uses_phpunit() {
    let preset = load_preset(Language::Php).unwrap();

    assert!(preset.quality_gate.test.contains("phpunit"));
    assert!(preset.quality_gate.lint.contains("phpstan"));
    assert!(preset.format.extensions.contains(&"php".to_string()));
}

#[test]
fn scala_preset_uses_sbt() {
    let preset = load_preset(Language::Scala).unwrap();

    assert!(preset.quality_gate.check.contains("sbt compile"));
    assert!(preset.quality_gate.test.contains("sbt test"));
    assert!(preset.format.extensions.contains(&"scala".to_string()));
}

// --- New framework override tests ---

#[test]
fn flutter_overlay_uses_flutter_commands() {
    let merged = load_preset_with_framework(Language::Dart, Framework::Flutter).unwrap();

    assert!(merged.quality_gate.check.contains("flutter analyze"));
    assert!(merged.quality_gate.test.contains("flutter test"));
}

#[test]
fn nestjs_overlay_uses_nest_build() {
    let merged = load_preset_with_framework(Language::TypeScript, Framework::NestJS).unwrap();

    assert!(merged.quality_gate.check.contains("nest build"));
}

#[test]
fn laravel_overlay_uses_artisan_test() {
    let merged = load_preset_with_framework(Language::Php, Framework::Laravel).unwrap();

    assert!(merged.quality_gate.test.contains("artisan test"));
}

#[test]
fn ktor_overlay_uses_gradlew() {
    let merged = load_preset_with_framework(Language::Kotlin, Framework::Ktor).unwrap();

    assert!(merged.quality_gate.check.contains("gradlew"));
    assert!(merged.quality_gate.test.contains("gradlew"));
}

#[test]
fn quarkus_overlay_uses_mvn() {
    let merged = load_preset_with_framework(Language::Java, Framework::Quarkus).unwrap();

    assert!(merged.quality_gate.check.contains("mvn compile"));
    assert!(merged.quality_gate.test.contains("mvn test"));
}

#[test]
fn micronaut_overlay_uses_mvn() {
    let merged = load_preset_with_framework(Language::Java, Framework::Micronaut).unwrap();

    assert!(merged.quality_gate.check.contains("mvn compile"));
}

#[test]
fn blazor_overlay_uses_warnaserror() {
    let merged = load_preset_with_framework(Language::CSharp, Framework::Blazor).unwrap();

    assert!(merged.quality_gate.check.contains("--warnaserror"));
}

#[test]
fn react_native_overlay_uses_tsc() {
    let merged = load_preset_with_framework(Language::TypeScript, Framework::ReactNative).unwrap();

    assert!(merged.quality_gate.check.contains("tsc"));
    assert!(merged.format.exclusions.contains(&"android".to_string()));
    assert!(merged.format.exclusions.contains(&"ios".to_string()));
}
