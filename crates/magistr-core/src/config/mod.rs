//! Configuration system — preset loading, migration, and validation.
//!
//! Presets are layered: built-in → framework → package-manager → database →
//! CI/CD → deployment → project-local → user override. Each layer is a TOML
//! or JSON file that partially overrides the previous layer.

pub mod agent_preferences;
pub mod fallback_preset;
pub mod inputs;
pub mod migration;
pub mod migration_check;
pub mod preset;
pub mod preset_fragment;
pub mod preset_framework;
pub mod preset_full;
pub mod preset_layers;
pub mod project_preset;
pub mod session_selection;
pub mod shareable;
pub mod user_config;
pub mod user_preset;
pub mod validate;
pub mod validation_warning;
