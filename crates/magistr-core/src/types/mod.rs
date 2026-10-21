//! Core domain types — language, framework, stack, config, presets, quality gates.
//!
//! All types are re-exported at the module level so consumers can use
//! `magistr_core::types::Language` or `magistr_core::Language` (via lib.rs re-exports).

mod archetype;
pub mod archetype_registry;
mod ci;
mod formatter;
mod framework;
mod language;
mod linter;
mod marker;
mod package_manager;
mod preset;
mod preset_layer;
mod project_config;
mod project_role;
mod project_tree;
mod quality_gate;
mod stack;
mod test_framework;
mod verification_config;
mod workspace;
pub use verification_config::{GatePolicy, VerificationConfig};

pub use archetype::{Archetype, ArchetypeRelationship, RelationshipKind, resolve_archetype};
pub use ci::CiProvider;
pub use formatter::Formatter;
pub use framework::Framework;
pub use language::Language;
pub use linter::Linter;
pub use magistr_engine::native::RoadmapBackend;
pub use marker::StackMarker;
pub use package_manager::PackageManager;
pub use preset::{
    FormatConfigOverride, Preset, PresetOverride, QualityGateOverride, TestingConfigOverride,
};
pub use preset_layer::{PresetLayer, PresetLayerType, PresetPipeline};
pub use project_config::{ProjectConfig, ProjectMeta, SpecConfig, StackConfig};
pub use project_role::ProjectRole;
pub use project_tree::{
    DetectedProject, EdgeKind, ProjectEdge, ProjectMetadata, ProjectTree, Topology,
};
pub use quality_gate::{CustomGate, FormatConfig, QualityGate, TestingConfig};
pub use stack::{PluginDetection, SmartStack, Stack};
pub use test_framework::TestFramework;
pub use workspace::Workspace;
