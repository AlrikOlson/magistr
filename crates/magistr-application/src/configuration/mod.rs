//! A typed configuration slice for setup and project defaults. File formats,
//! config discovery and layer merging are owned by the configuration adapter.

mod configuration_owner;
mod configuration_port;
mod configuration_snapshot;
mod configuration_update;
mod project_settings;
mod resolved_value;
mod service;
mod setup_project;

pub use configuration_owner::ConfigurationOwner;
pub use configuration_port::ConfigurationPort;
pub use configuration_snapshot::ConfigurationSnapshot;
pub use configuration_update::ConfigurationUpdate;
pub use project_settings::ProjectSettings;
pub use resolved_value::ResolvedValue;
pub use service::Configuration;
pub use setup_project::SetupProject;
