//! Preset loading with framework and package-manager overlays.

use crate::error::{MagistrError, Result};
use crate::types::{Framework, Language, PackageManager, Preset, PresetOverride};

/// Embedded framework overlay presets.
const NEXTJS_OVERRIDE: &str = include_str!("../../presets/overrides/nextjs.json");
const VUE_OVERRIDE: &str = include_str!("../../presets/overrides/vue.json");
const SVELTEKIT_OVERRIDE: &str = include_str!("../../presets/overrides/sveltekit.json");
const EXPRESS_OVERRIDE: &str = include_str!("../../presets/overrides/express.json");
const DJANGO_OVERRIDE: &str = include_str!("../../presets/overrides/django.json");
const FASTAPI_OVERRIDE: &str = include_str!("../../presets/overrides/fastapi.json");
const FLASK_OVERRIDE: &str = include_str!("../../presets/overrides/flask.json");
const AXUM_OVERRIDE: &str = include_str!("../../presets/overrides/axum.json");
const BEVY_OVERRIDE: &str = include_str!("../../presets/overrides/bevy.json");
const TAURI_OVERRIDE: &str = include_str!("../../presets/overrides/tauri.json");
const GIN_OVERRIDE: &str = include_str!("../../presets/overrides/gin.json");
const RAILS_OVERRIDE: &str = include_str!("../../presets/overrides/rails.json");
const PHOENIX_OVERRIDE: &str = include_str!("../../presets/overrides/phoenix.json");
const SPRING_OVERRIDE: &str = include_str!("../../presets/overrides/spring.json");
const VAPOR_OVERRIDE: &str = include_str!("../../presets/overrides/vapor.json");
const ASPNET_OVERRIDE: &str = include_str!("../../presets/overrides/aspnet.json");
const BLAZOR_OVERRIDE: &str = include_str!("../../presets/overrides/blazor.json");
const MAUI_OVERRIDE: &str = include_str!("../../presets/overrides/maui.json");
const FLUTTER_OVERRIDE: &str = include_str!("../../presets/overrides/flutter.json");
const REACT_NATIVE_OVERRIDE: &str = include_str!("../../presets/overrides/react-native.json");
const MICRONAUT_OVERRIDE: &str = include_str!("../../presets/overrides/micronaut.json");
const NESTJS_OVERRIDE: &str = include_str!("../../presets/overrides/nestjs.json");
const LARAVEL_OVERRIDE: &str = include_str!("../../presets/overrides/laravel.json");
const KTOR_OVERRIDE: &str = include_str!("../../presets/overrides/ktor.json");
const QUARKUS_OVERRIDE: &str = include_str!("../../presets/overrides/quarkus.json");

/// Embedded package-manager overlay presets.
const GRADLE_OVERRIDE: &str = include_str!("../../presets/overrides/gradle.json");
const BUN_OVERRIDE: &str = include_str!("../../presets/overrides/bun.json");

/// Load a language preset with a framework overlay merged on top.
///
/// If no overlay exists for the framework, returns the base language preset unchanged.
pub fn load_preset_with_framework(lang: Language, framework: Framework) -> Result<Preset> {
    let base = super::preset::load_preset(lang)?;

    match framework_override_json(framework) {
        Some(json) => {
            let overlay: PresetOverride = serde_json::from_str(json)
                .map_err(|e| MagistrError::Config(format!("overlay parse error: {e}")))?;
            Ok(base.merge(&overlay))
        }
        None => Ok(base),
    }
}

/// Apply a package-manager overlay on top of an already-resolved preset.
///
/// This handles cases where the build tool (e.g. Gradle vs Maven) changes
/// quality gate commands independently of the framework.
pub fn apply_package_manager_overlay(
    preset: Preset,
    package_manager: PackageManager,
) -> Result<Preset> {
    match pm_override_json(package_manager) {
        Some(json) => {
            let overlay: PresetOverride = serde_json::from_str(json)
                .map_err(|e| MagistrError::Config(format!("pm overlay parse error: {e}")))?;
            Ok(preset.merge(&overlay))
        }
        None => Ok(preset),
    }
}

/// Public accessor for the raw framework override JSON (used by the pipeline).
pub fn framework_override_json_pub(framework: Framework) -> Option<&'static str> {
    framework_override_json(framework)
}

/// Public accessor for the raw package-manager override JSON (used by the pipeline).
pub fn pm_override_json_pub(package_manager: PackageManager) -> Option<&'static str> {
    pm_override_json(package_manager)
}

/// Registry of framework → override JSON mappings.
///
/// Adding a new framework override:
/// 1. Add the `include_str!` constant above
/// 2. Add one entry to this array
///    No match statement changes needed (OCP).
const FRAMEWORK_REGISTRY: &[(Framework, &str)] = &[
    // JS/TS
    (Framework::Next, NEXTJS_OVERRIDE),
    (Framework::Vue, VUE_OVERRIDE),
    (Framework::Nuxt, VUE_OVERRIDE),
    (Framework::SvelteKit, SVELTEKIT_OVERRIDE),
    (Framework::Express, EXPRESS_OVERRIDE),
    (Framework::Fastify, EXPRESS_OVERRIDE),
    (Framework::Hono, EXPRESS_OVERRIDE),
    (Framework::NestJS, NESTJS_OVERRIDE),
    // Python
    (Framework::Django, DJANGO_OVERRIDE),
    (Framework::FastApi, FASTAPI_OVERRIDE),
    (Framework::Flask, FLASK_OVERRIDE),
    // Rust
    (Framework::Axum, AXUM_OVERRIDE),
    (Framework::Actix, AXUM_OVERRIDE),
    (Framework::Rocket, AXUM_OVERRIDE),
    (Framework::Tauri, TAURI_OVERRIDE),
    (Framework::Bevy, BEVY_OVERRIDE),
    (Framework::Leptos, BEVY_OVERRIDE),
    (Framework::Dioxus, BEVY_OVERRIDE),
    // Go
    (Framework::Gin, GIN_OVERRIDE),
    (Framework::Echo, GIN_OVERRIDE),
    (Framework::Fiber, GIN_OVERRIDE),
    (Framework::Chi, GIN_OVERRIDE),
    // Ruby
    (Framework::Rails, RAILS_OVERRIDE),
    // Elixir
    (Framework::Phoenix, PHOENIX_OVERRIDE),
    // Java/Kotlin
    (Framework::Spring, SPRING_OVERRIDE),
    (Framework::Quarkus, QUARKUS_OVERRIDE),
    (Framework::Ktor, KTOR_OVERRIDE),
    (Framework::Micronaut, MICRONAUT_OVERRIDE),
    // Swift
    (Framework::Vapor, VAPOR_OVERRIDE),
    // C#
    (Framework::AspNet, ASPNET_OVERRIDE),
    (Framework::Blazor, BLAZOR_OVERRIDE),
    (Framework::Maui, MAUI_OVERRIDE),
    // Dart
    (Framework::Flutter, FLUTTER_OVERRIDE),
    // Mobile
    (Framework::ReactNative, REACT_NATIVE_OVERRIDE),
    // PHP
    (Framework::Laravel, LARAVEL_OVERRIDE),
];

fn framework_override_json(framework: Framework) -> Option<&'static str> {
    FRAMEWORK_REGISTRY
        .iter()
        .find(|(f, _)| *f == framework)
        .map(|(_, json)| *json)
}

/// Registry of package-manager → override JSON mappings.
const PM_REGISTRY: &[(PackageManager, &str)] = &[
    (PackageManager::Gradle, GRADLE_OVERRIDE),
    (PackageManager::Bun, BUN_OVERRIDE),
];

fn pm_override_json(package_manager: PackageManager) -> Option<&'static str> {
    PM_REGISTRY
        .iter()
        .find(|(pm, _)| *pm == package_manager)
        .map(|(_, json)| *json)
}
