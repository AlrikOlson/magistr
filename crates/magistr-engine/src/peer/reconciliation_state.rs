use super::ReconciliationStop;

/// State as of the latest host observation; Agreed is not dispatch authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconciliationState {
    Collecting,
    Reviewing,
    Agreed,
    Stopped(ReconciliationStop),
}
