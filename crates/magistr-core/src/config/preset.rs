pub use super::fallback_preset::fallback_preset;
pub use super::preset_framework::load_preset_with_framework;
pub use super::preset_full::{LayerOptions, load_preset_layered};

use crate::error::{MagistrError, Result};
use crate::types::{Language, Preset};

/// Embedded preset JSON files, compiled into the binary.
const RUST_PRESET: &str = include_str!("../../presets/rust.json");
const TYPESCRIPT_PRESET: &str = include_str!("../../presets/typescript.json");
const PYTHON_PRESET: &str = include_str!("../../presets/python.json");
const GO_PRESET: &str = include_str!("../../presets/go.json");
const JAVA_PRESET: &str = include_str!("../../presets/java.json");
const KOTLIN_PRESET: &str = include_str!("../../presets/kotlin.json");
const RUBY_PRESET: &str = include_str!("../../presets/ruby.json");
const ELIXIR_PRESET: &str = include_str!("../../presets/elixir.json");
const SWIFT_PRESET: &str = include_str!("../../presets/swift.json");
const CPP_PRESET: &str = include_str!("../../presets/cpp.json");
const CSHARP_PRESET: &str = include_str!("../../presets/csharp.json");
const DART_PRESET: &str = include_str!("../../presets/dart.json");
const PHP_PRESET: &str = include_str!("../../presets/php.json");
const SCALA_PRESET: &str = include_str!("../../presets/scala.json");

/// Load the embedded preset for a given language.
pub fn load_preset(lang: Language) -> Result<Preset> {
    let json = preset_json(lang)?;
    serde_json::from_str(json).map_err(|e| MagistrError::Config(format!("preset parse error: {e}")))
}

fn preset_json(lang: Language) -> Result<&'static str> {
    match lang {
        Language::Rust => Ok(RUST_PRESET),
        Language::TypeScript | Language::JavaScript => Ok(TYPESCRIPT_PRESET),
        Language::Python => Ok(PYTHON_PRESET),
        Language::Go => Ok(GO_PRESET),
        Language::Java => Ok(JAVA_PRESET),
        Language::Kotlin => Ok(KOTLIN_PRESET),
        Language::Ruby => Ok(RUBY_PRESET),
        Language::Elixir => Ok(ELIXIR_PRESET),
        Language::Swift => Ok(SWIFT_PRESET),
        Language::Cpp => Ok(CPP_PRESET),
        Language::CSharp => Ok(CSHARP_PRESET),
        Language::Dart => Ok(DART_PRESET),
        Language::Php => Ok(PHP_PRESET),
        Language::Scala => Ok(SCALA_PRESET),
        Language::Unknown => Err(MagistrError::PresetNotFound("unknown".to_string())),
    }
}

#[cfg(test)]
#[path = "preset.tests.rs"]
mod tests;
