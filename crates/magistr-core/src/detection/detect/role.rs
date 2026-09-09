//! Project role classification heuristics.
//!
//! Classifies each detected project as frontend, backend, fullstack, library,
//! CLI, service, or unknown based on markers, language, framework, and
//! dependency analysis.

use std::path::Path;

use crate::types::{Framework, Language, ProjectRole, StackMarker};

/// Frontend SPA/static-site frameworks.
const FRONTEND_FRAMEWORKS: &[Framework] = &[
    Framework::React,
    Framework::Vue,
    Framework::Angular,
    Framework::SvelteKit,
    Framework::Astro,
    Framework::Vite,
];

/// Backend/server frameworks.
const BACKEND_FRAMEWORKS: &[Framework] = &[
    Framework::Express,
    Framework::Fastify,
    Framework::Hono,
    Framework::NestJS,
    Framework::Django,
    Framework::FastApi,
    Framework::Flask,
    Framework::Rails,
    Framework::Sinatra,
    Framework::Phoenix,
    Framework::Axum,
    Framework::Actix,
    Framework::Rocket,
    Framework::Spring,
    Framework::Quarkus,
    Framework::Ktor,
    Framework::Vapor,
    Framework::AspNet,
    Framework::Gin,
    Framework::Echo,
    Framework::Fiber,
    Framework::Chi,
    Framework::Laravel,
    Framework::Micronaut,
];

/// Fullstack frameworks (both frontend and backend).
const FULLSTACK_FRAMEWORKS: &[Framework] = &[
    Framework::Next,
    Framework::Nuxt,
    Framework::Remix,
    Framework::Blazor,
];

/// CLI-indicating dependencies.
const CLI_DEPS: &[&str] = &[
    "clap",
    "commander",
    "yargs",
    "argparse",
    "click",
    "typer",
    "cobra",
    "urfave/cli",
    "structopt",
    "argh",
];

/// Classify a project's role based on its detected characteristics.
pub fn classify_role(
    _dir: &Path,
    language: Language,
    framework: Option<Framework>,
    markers: &[StackMarker],
    dependencies: &[String],
    is_docker_compose_service: bool,
) -> ProjectRole {
    // Service: referenced in docker-compose
    if is_docker_compose_service {
        // Still check if it's specifically frontend/backend
        if let Some(fw) = framework {
            if FRONTEND_FRAMEWORKS.contains(&fw) {
                return ProjectRole::Frontend;
            }
            if BACKEND_FRAMEWORKS.contains(&fw) {
                return ProjectRole::Service;
            }
        }
        return ProjectRole::Service;
    }

    // Framework-based classification
    if let Some(fw) = framework {
        if fw == Framework::None {
            // No framework detected, use other heuristics below
        } else if FULLSTACK_FRAMEWORKS.contains(&fw) {
            return ProjectRole::Fullstack;
        } else if FRONTEND_FRAMEWORKS.contains(&fw) {
            return ProjectRole::Frontend;
        } else if BACKEND_FRAMEWORKS.contains(&fw) {
            return ProjectRole::Backend;
        } else if matches!(
            fw,
            Framework::Tauri | Framework::Maui | Framework::Flutter | Framework::ReactNative
        ) {
            return ProjectRole::Frontend; // Desktop/mobile apps
        } else if fw == Framework::Bevy {
            return ProjectRole::Frontend; // Game engine
        } else if fw == Framework::Leptos || fw == Framework::Dioxus {
            return ProjectRole::Fullstack; // Rust fullstack
        }
    }

    // CLI detection via dependencies
    let has_cli_dep = dependencies
        .iter()
        .any(|dep| CLI_DEPS.iter().any(|cli| dep.contains(cli)));
    if has_cli_dep {
        return ProjectRole::Cli;
    }

    // Library detection: workspace member without binary entry points
    let is_library = match language {
        Language::Rust => {
            // Check if it lacks a src/main.rs (library-only crate)
            markers.contains(&StackMarker::CargoToml) && !has_binary_marker(language, markers)
        }
        Language::TypeScript | Language::JavaScript => {
            // Packages that are workspace members without a "start" script or server framework
            // are likely libraries
            false // Conservative: can't determine without more context
        }
        _ => false,
    };

    if is_library {
        return ProjectRole::Library;
    }

    ProjectRole::Unknown
}

/// Check if the project has indicators of being a binary (not a library).
fn has_binary_marker(_language: Language, _markers: &[StackMarker]) -> bool {
    // This is a simplified check. In practice, we'd look at
    // Cargo.toml [[bin]] targets or src/main.rs existence.
    // For now, return false to be conservative.
    false
}

#[cfg(test)]
#[path = "role.tests.rs"]
mod tests;
