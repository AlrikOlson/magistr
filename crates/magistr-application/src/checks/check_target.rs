use alloc::string::String;
use magistr_domain::gate::GateKind;

use crate::ApplicationError;

/// Exact configured gate identity and its result category. Names retain custom
/// gates and archetype suffixes such as `test:app-typescript`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckTarget {
    kind: GateKind,
    name: String,
}

impl CheckTarget {
    pub fn new(kind: GateKind, name: impl Into<String>) -> Result<Self, ApplicationError> {
        let name = name.into();
        if name.trim().is_empty() || name.chars().any(char::is_control) {
            return Err(ApplicationError::InvalidRequest {
                reason: "configured gate name must be nonempty and contain no control characters"
                    .into(),
            });
        }
        Ok(Self { kind, name })
    }

    pub fn standard(kind: GateKind) -> Self {
        let name = match kind {
            GateKind::Check => "check",
            GateKind::Test => "test",
            GateKind::Lint => "lint",
        };
        Self {
            kind,
            name: name.into(),
        }
    }

    pub fn kind(&self) -> GateKind {
        self.kind
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
