use crate::worker::{WorkerGuestEnd, WorkerLanguageRegistration};

/// A lang-pack carrier read plus the worker's registration verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LangPackLoadOutcome {
    pub end: WorkerGuestEnd,
    pub fuel_used: u64,
    /// `None` means the pack did not register (the end says why).
    pub registration: Option<WorkerLanguageRegistration>,
}
