use std::path::Path;

use tracing::instrument;

use super::config_parser::ManifestData;
use crate::types::{Framework, Language};

/// Detect the framework based on language, config files, and parsed manifest data.
#[instrument(skip_all, fields(%lang))]
pub fn detect_framework(dir: &Path, lang: Language, manifest: &ManifestData) -> Framework {
    match lang {
        Language::TypeScript | Language::JavaScript => detect_js_framework(dir, manifest),
        Language::Python => detect_python_framework(dir, manifest),
        Language::Ruby => detect_ruby_framework(dir, manifest),
        Language::Elixir => detect_elixir_framework(manifest),
        Language::Rust => detect_rust_framework(dir, manifest),
        Language::Go => detect_go_framework(dir, manifest),
        Language::Java => detect_java_framework(dir),
        Language::Kotlin => detect_kotlin_framework(dir),
        Language::Swift => detect_swift_framework(dir),
        Language::CSharp => detect_csharp_framework(dir),
        Language::Dart => detect_dart_framework(dir, manifest),
        Language::Php => detect_php_framework(dir, manifest),
        _ => Framework::None,
    }
}

fn detect_js_framework(dir: &Path, manifest: &ManifestData) -> Framework {
    // Config file checks (highest confidence)
    for name in ["next.config.js", "next.config.ts", "next.config.mjs"] {
        if dir.join(name).exists() {
            return Framework::Next;
        }
    }
    for name in ["nuxt.config.ts", "nuxt.config.js"] {
        if dir.join(name).exists() {
            return Framework::Nuxt;
        }
    }
    if dir.join("svelte.config.js").exists() {
        return Framework::SvelteKit;
    }
    for name in ["astro.config.mjs", "astro.config.ts"] {
        if dir.join(name).exists() {
            return Framework::Astro;
        }
    }
    for name in ["remix.config.js", "remix.config.ts"] {
        if dir.join(name).exists() {
            return Framework::Remix;
        }
    }
    for name in ["vite.config.ts", "vite.config.js"] {
        if dir.join(name).exists() {
            return Framework::Vite;
        }
    }
    if dir.join("angular.json").exists() {
        return Framework::Angular;
    }

    // Fall back to parsed package.json dependency checks
    for (dep, fw) in [
        ("next", Framework::Next),
        ("nuxt", Framework::Nuxt),
        ("@nestjs/core", Framework::NestJS),
        ("react-native", Framework::ReactNative),
        ("express", Framework::Express),
        ("fastify", Framework::Fastify),
        ("hono", Framework::Hono),
        ("react", Framework::React),
        ("vue", Framework::Vue),
    ] {
        if manifest.has_dep(dep) {
            return fw;
        }
    }

    Framework::None
}

fn detect_python_framework(dir: &Path, manifest: &ManifestData) -> Framework {
    if dir.join("manage.py").exists() {
        return Framework::Django;
    }

    // Use parsed dependency data for precise matching
    if manifest.has_dep("django") {
        return Framework::Django;
    }
    if manifest.has_dep("fastapi") {
        return Framework::FastApi;
    }
    if manifest.has_dep("flask") {
        return Framework::Flask;
    }

    Framework::None
}

fn detect_ruby_framework(dir: &Path, manifest: &ManifestData) -> Framework {
    if dir.join("config/routes.rb").exists() {
        return Framework::Rails;
    }
    if manifest.has_dep("rails") {
        return Framework::Rails;
    }
    if manifest.has_dep("sinatra") {
        return Framework::Sinatra;
    }
    Framework::None
}

fn detect_elixir_framework(manifest: &ManifestData) -> Framework {
    if manifest.has_dep("phoenix") {
        return Framework::Phoenix;
    }
    Framework::None
}

fn detect_rust_framework(dir: &Path, manifest: &ManifestData) -> Framework {
    // Tauri has a config file — highest confidence
    if dir.join("tauri.conf.json").exists() || dir.join("src-tauri/tauri.conf.json").exists() {
        return Framework::Tauri;
    }

    // Check parsed dependencies — order by specificity (app frameworks before libraries)
    for (dep, fw) in [
        ("tauri", Framework::Tauri),
        ("leptos", Framework::Leptos),
        ("dioxus", Framework::Dioxus),
        ("bevy", Framework::Bevy),
        ("axum", Framework::Axum),
        ("actix-web", Framework::Actix),
        ("rocket", Framework::Rocket),
    ] {
        if manifest.has_dep(dep) {
            return fw;
        }
    }

    Framework::None
}

fn detect_java_framework(dir: &Path) -> Framework {
    // Check pom.xml for Spring Boot, Quarkus, or Micronaut
    if let Ok(pom) = std::fs::read_to_string(dir.join("pom.xml")) {
        if pom.contains("spring-boot") {
            return Framework::Spring;
        }
        if pom.contains("quarkus") {
            return Framework::Quarkus;
        }
        if pom.contains("micronaut") {
            return Framework::Micronaut;
        }
    }

    // Check build.gradle or build.gradle.kts
    for name in ["build.gradle.kts", "build.gradle"] {
        if let Ok(gradle) = std::fs::read_to_string(dir.join(name)) {
            if gradle.contains("spring-boot") || gradle.contains("org.springframework.boot") {
                return Framework::Spring;
            }
            if gradle.contains("quarkus") {
                return Framework::Quarkus;
            }
            if gradle.contains("micronaut") {
                return Framework::Micronaut;
            }
        }
    }

    // Config file fallback — Spring Boot convention
    if dir
        .join("src/main/resources/application.properties")
        .exists()
        || dir.join("src/main/resources/application.yml").exists()
    {
        return Framework::Spring;
    }

    Framework::None
}

fn detect_kotlin_framework(dir: &Path) -> Framework {
    if let Ok(gradle) = std::fs::read_to_string(dir.join("build.gradle.kts")) {
        // Ktor before Spring — more Kotlin-specific
        if gradle.contains("io.ktor") {
            return Framework::Ktor;
        }
        if gradle.contains("spring-boot") || gradle.contains("org.springframework.boot") {
            return Framework::Spring;
        }
        if gradle.contains("quarkus") {
            return Framework::Quarkus;
        }
    }

    if let Ok(gradle) = std::fs::read_to_string(dir.join("build.gradle")) {
        if gradle.contains("io.ktor") {
            return Framework::Ktor;
        }
        if gradle.contains("spring-boot") || gradle.contains("org.springframework.boot") {
            return Framework::Spring;
        }
    }

    Framework::None
}

fn detect_swift_framework(dir: &Path) -> Framework {
    if std::fs::read_to_string(dir.join("Package.swift"))
        .is_ok_and(|contents| contents.contains("vapor"))
    {
        return Framework::Vapor;
    }
    Framework::None
}

fn detect_csharp_framework(dir: &Path) -> Framework {
    // Check .csproj files for specific frameworks
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.ends_with(".csproj")
                && let Ok(csproj) = std::fs::read_to_string(entry.path())
            {
                // MAUI before Blazor — more specific
                if csproj.contains("Microsoft.Maui")
                    || csproj.contains("net8.0-android")
                    || csproj.contains("net8.0-ios")
                {
                    return Framework::Maui;
                }
                if csproj.contains("Microsoft.AspNetCore.Components") && csproj.contains("Blazor") {
                    return Framework::Blazor;
                }
                if csproj.contains("Microsoft.AspNetCore")
                    || csproj.contains("Microsoft.NET.Sdk.Web")
                {
                    return Framework::AspNet;
                }
            }
        }
    }

    // Fallback: ASP.NET convention (Program.cs + appsettings.json)
    if dir.join("Program.cs").exists() && dir.join("appsettings.json").exists() {
        return Framework::AspNet;
    }

    Framework::None
}

fn detect_go_framework(dir: &Path, manifest: &ManifestData) -> Framework {
    // Use parsed go.mod data — manifest stores both full module paths and last segments
    for (dep, fw) in [
        ("github.com/gin-gonic/gin", Framework::Gin),
        ("github.com/labstack/echo", Framework::Echo),
        ("github.com/gofiber/fiber", Framework::Fiber),
        ("github.com/go-chi/chi", Framework::Chi),
    ] {
        if manifest.has_dep(dep) {
            return fw;
        }
    }

    // Fallback: check versioned paths (e.g., github.com/go-chi/chi/v5)
    if !manifest.dependencies.is_empty() {
        for dep_name in &manifest.dependencies {
            if dep_name.contains("gin-gonic/gin") {
                return Framework::Gin;
            }
            if dep_name.contains("labstack/echo") {
                return Framework::Echo;
            }
            if dep_name.contains("gofiber/fiber") {
                return Framework::Fiber;
            }
            if dep_name.contains("go-chi/chi") {
                return Framework::Chi;
            }
        }
    }

    // Last resort: raw file read for edge cases the parser might miss
    let go_mod = match std::fs::read_to_string(dir.join("go.mod")) {
        Ok(s) => s,
        Err(_) => return Framework::None,
    };

    if go_mod.contains("gin-gonic/gin") {
        return Framework::Gin;
    }
    if go_mod.contains("labstack/echo") {
        return Framework::Echo;
    }
    if go_mod.contains("gofiber/fiber") {
        return Framework::Fiber;
    }
    if go_mod.contains("go-chi/chi") {
        return Framework::Chi;
    }

    Framework::None
}

fn detect_dart_framework(dir: &Path, manifest: &ManifestData) -> Framework {
    // Flutter projects have a flutter SDK dependency in pubspec.yaml
    if manifest.has_dep("flutter") {
        return Framework::Flutter;
    }

    // Fallback: check pubspec.yaml for flutter dependency
    if let Ok(pubspec) = std::fs::read_to_string(dir.join("pubspec.yaml"))
        && pubspec.contains("flutter:")
        && pubspec.contains("sdk: flutter")
    {
        return Framework::Flutter;
    }

    Framework::None
}

fn detect_php_framework(dir: &Path, manifest: &ManifestData) -> Framework {
    // Laravel has artisan CLI
    if dir.join("artisan").exists() {
        return Framework::Laravel;
    }

    // Check composer.json dependencies
    if manifest.has_dep("laravel/framework") {
        return Framework::Laravel;
    }

    // Fallback: raw file read
    if let Ok(composer) = std::fs::read_to_string(dir.join("composer.json"))
        && composer.contains("laravel/framework")
    {
        return Framework::Laravel;
    }

    Framework::None
}

#[cfg(test)]
#[path = "framework.tests.rs"]
mod tests;
