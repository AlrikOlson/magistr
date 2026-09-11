use serde::{Deserialize, Serialize};

use super::WorkerLanguageLane;

/// Registration provenance reported from the worker's own grammar registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerLanguageRegistration {
    pub id: String,
    pub extensions: Vec<String>,
    pub lane: WorkerLanguageLane,
    pub queries_ok: bool,
    pub notes: Vec<String>,
}

impl From<crate::lang_registry::RegistrationOutcome> for WorkerLanguageRegistration {
    fn from(registration: crate::lang_registry::RegistrationOutcome) -> Self {
        Self {
            id: registration.id,
            extensions: registration.extensions,
            lane: match registration.lane {
                crate::lang_registry::Lane::Native => WorkerLanguageLane::Native,
                crate::lang_registry::Lane::Wasm => WorkerLanguageLane::Wasm,
            },
            queries_ok: registration.queries_ok,
            notes: registration.notes,
        }
    }
}
