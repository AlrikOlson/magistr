use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Framework {
    // JS/TS
    Next,
    Nuxt,
    #[serde(alias = "sveltekit")]
    SvelteKit,
    Astro,
    Remix,
    Vite,
    Angular,
    Express,
    Fastify,
    Hono,
    React,
    Vue,
    // Python
    Django,
    #[serde(alias = "fastapi")]
    FastApi,
    Flask,
    // Ruby
    Rails,
    Sinatra,
    // Elixir
    Phoenix,
    // Rust
    Axum,
    Actix,
    Rocket,
    Tauri,
    Leptos,
    Dioxus,
    Bevy,
    // Go
    Gin,
    Echo,
    Fiber,
    Chi,
    // Java/Kotlin
    Spring,
    Quarkus,
    Ktor,
    // Swift
    Vapor,
    // C#
    #[serde(alias = "aspnet")]
    AspNet,
    Blazor,
    Maui,
    // Dart
    Flutter,
    // Mobile
    #[serde(alias = "reactnative", alias = "react-native")]
    ReactNative,
    // Java (additional)
    Micronaut,
    // JS/TS (additional)
    #[serde(alias = "nestjs")]
    NestJS,
    // PHP
    Laravel,
    // None
    None,
}

impl Framework {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Next => "next",
            Self::Nuxt => "nuxt",
            Self::SvelteKit => "sveltekit",
            Self::Astro => "astro",
            Self::Remix => "remix",
            Self::Vite => "vite",
            Self::Angular => "angular",
            Self::Express => "express",
            Self::Fastify => "fastify",
            Self::Hono => "hono",
            Self::React => "react",
            Self::Vue => "vue",
            Self::Django => "django",
            Self::FastApi => "fastapi",
            Self::Flask => "flask",
            Self::Rails => "rails",
            Self::Sinatra => "sinatra",
            Self::Phoenix => "phoenix",
            Self::Axum => "axum",
            Self::Actix => "actix",
            Self::Rocket => "rocket",
            Self::Tauri => "tauri",
            Self::Leptos => "leptos",
            Self::Dioxus => "dioxus",
            Self::Bevy => "bevy",
            Self::Gin => "gin",
            Self::Echo => "echo",
            Self::Fiber => "fiber",
            Self::Chi => "chi",
            Self::Spring => "spring",
            Self::Quarkus => "quarkus",
            Self::Ktor => "ktor",
            Self::Vapor => "vapor",
            Self::AspNet => "aspnet",
            Self::Blazor => "blazor",
            Self::Maui => "maui",
            Self::Flutter => "flutter",
            Self::ReactNative => "reactnative",
            Self::Micronaut => "micronaut",
            Self::NestJS => "nestjs",
            Self::Laravel => "laravel",
            Self::None => "none",
        }
    }
}

impl std::fmt::Display for Framework {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
