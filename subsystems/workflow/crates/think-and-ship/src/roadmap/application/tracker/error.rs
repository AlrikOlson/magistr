/// Stable classification of a tracker setup failure.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct TrackerSetupError {
    pub kind: &'static str,
    pub message: String,
}

impl TrackerSetupError {
    pub(crate) fn new(kind: &'static str, message: impl ToString) -> Self {
        Self {
            kind,
            message: message.to_string(),
        }
    }
}
