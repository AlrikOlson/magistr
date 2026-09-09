//! Scoped identities allocated by application adapters, never by the domain.

mod attempt_id;
mod command_id;
mod decision_id;
mod identity_error;
mod identity_key;
mod project_id;
mod provider_session_binding;
mod provider_session_id;
mod run_id;
mod session_id;
mod turn_id;

pub use attempt_id::AttemptId;
pub use command_id::CommandId;
pub use decision_id::DecisionId;
pub use identity_error::IdentityError;
pub use project_id::ProjectId;
pub use provider_session_binding::ProviderSessionBinding;
pub use provider_session_id::ProviderSessionId;
pub use run_id::RunId;
pub use session_id::SessionId;
pub use turn_id::TurnId;

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
