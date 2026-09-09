use magistr_domain::identity::ProjectId;

use crate::{CommandReceipt, PortFuture};

use super::{ConfigurationSnapshot, ConfigurationUpdate, SetupProject};

/// Resolves layered configuration and persists only the requested project patch.
///
/// The host enforces `ConfigureProject` and compares the project-write revision
/// atomically with persistence. It must preserve unmentioned settings, ownership
/// of other layers, project identity and export/policy controls. A resolved read
/// is never serialized wholesale as the project file. Returned revisions use
/// the common project authority's state history, never a file-local counter or
/// stream cursor. Checking a config file alone is insufficient admission.
/// A receipt confirms the project patch was persisted, not that a higher layer
/// stopped overriding it: callers read again for the resolved effective values.
pub trait ConfigurationPort: Send {
    fn read(&mut self, project: &ProjectId) -> PortFuture<'_, ConfigurationSnapshot>;

    /// Check the absent-project-config precondition atomically with creation,
    /// returning a conflict/refusal if configuration already exists. Preserve
    /// user/invocation settings and initialize the configured native export and
    /// ownership policy through the host; never infer ownership from a path.
    fn setup(&mut self, request: SetupProject) -> PortFuture<'_, CommandReceipt>;

    fn update(&mut self, request: ConfigurationUpdate) -> PortFuture<'_, CommandReceipt>;
}
