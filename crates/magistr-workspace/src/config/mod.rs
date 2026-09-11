//! Workspace-tool configuration and command admission, independent of Magistr's
//! project preferences and permission-policy owners. Resolution only reads;
//! detection suggests commands and never amends the execution allowlist.

mod cargo_layout;
mod command_admission;
mod detected_command;
mod key_spec;
mod layer;
mod layer_report;
mod policy_violation;
mod resolve_options;
mod resolved_config;
mod resolved_key;
mod session_flags;
mod settings;

pub use command_admission::CommandAdmission;
pub use detected_command::DetectedCommand;
pub use layer::ConfigLayer;
pub use layer_report::LayerReport;
pub use policy_violation::PolicyViolation;
pub use resolve_options::ResolveOptions;
pub use resolved_config::ResolvedConfig;
pub use resolved_key::ResolvedKey;
pub use session_flags::SessionFlags;
