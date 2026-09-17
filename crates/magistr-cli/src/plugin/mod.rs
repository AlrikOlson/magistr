//! Plugin/extension system for magistr.
//!
//! Plugins are directories containing a `magistr-plugin.toml` manifest
//! and associated resources. Four kinds are supported:
//!
//! - **Detection plugins** — custom rules for detecting internal frameworks,
//!   proprietary stacks, or team-specific tools
//! - **Template plugins** — custom scaffold templates (rules, hooks, skills)
//!   that get copied into `.claude/` during `magistr init`
//! - **Workflow-step plugins** — custom steps in the iteration loop
//! - **Quality-gate plugins** — custom quality gates (SonarQube, license checks)
//!
//! Plugins are tools, never agents: a model is reached over ACP only, served
//! by `magistr acp-agent --provider <p>` or any native ACP agent on PATH. A
//! manifest still declaring the retired `backend` kind is refused with that
//! pointer ([`PluginManifest::from_toml`]).
//!
//! Plugins are installed to `~/.config/magistr/plugins/` and managed via
//! `magistr plugins list|add|remove`.
//!
//! ## Executable Plugin Protocol
//!
//! Executable plugins (workflow-step, quality-gate) run as subprocesses
//! and communicate via JSON-line protocol over stdin/stdout. The lifecycle is:
//!
//! 1. **Spawn** — magistr starts the plugin subprocess
//! 2. **Init** — magistr sends host metadata, plugin reports capabilities
//! 3. **Activate** — magistr provides project context, plugin prepares
//! 4. *(use — workflow/gate operations)*
//! 5. **Deactivate** — graceful shutdown

pub mod discovery;
pub mod gate_trait;
pub mod gates;
pub mod index;
pub mod lifecycle;
pub mod loader;
pub mod process;
pub mod registry;
pub mod signing;
pub mod storage_trait;
pub mod types;
pub mod workflow_step_trait;

#[allow(unused_imports)]
pub use discovery::{
    discover_global_plugins, discover_local_plugins, discover_plugins, local_plugins_dir,
};
#[allow(unused_imports)]
pub use gate_trait::{
    GateConfig, GateFinding, GateOutcome, GatePlugin, GateSeverity, SubprocessGate,
};
#[allow(unused_imports)]
pub use gates::{license::LicenseGate, sbom::SbomGate, sonarqube::SonarQubeGate};
#[allow(unused_imports)]
pub use index::{IndexEntry, RegistryIndex, download_archive, extract_archive};
#[allow(unused_imports)]
pub use lifecycle::{PluginInstance, PluginState};
#[allow(unused_imports)]
pub use loader::{load_detection_config, load_manifest, load_template_config, validate_plugin};
#[allow(unused_imports)]
pub use process::PluginProcess;
#[allow(unused_imports)]
pub use registry::{OrgPluginConfig, PluginRegistry};
#[allow(unused_imports)]
pub use signing::verify_signature;
#[allow(unused_imports)]
pub use storage_trait::{StoragePlugin, StorageRecord};
#[allow(unused_imports)]
pub use types::{
    DetectionConfig, DetectionRule, DiscoveredPlugin, InstalledPlugin, PluginCompatibility,
    PluginDependency, PluginError, PluginKind, PluginManifest, PluginPermission, PluginRequest,
    PluginResponse, PluginScope, PluginSource, TemplateConfig, TemplateFile,
};
#[allow(unused_imports)]
pub use workflow_step_trait::{
    StepConfig, StepFinding, StepOutcome, StepSeverity, SubprocessWorkflowStep, WorkflowPhase,
    WorkflowStepPlugin,
};
