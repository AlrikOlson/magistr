use std::fmt;

/// Logical refusal independent of the transport's error envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipError {
    pub kind: &'static str,
    pub message: String,
}

impl ShipError {
    pub(crate) fn invalid_args(message: impl Into<String>) -> Self {
        Self {
            kind: "invalid_args",
            message: message.into(),
        }
    }
}

impl From<String> for ShipError {
    fn from(message: String) -> Self {
        Self {
            kind: "invalid_state",
            message,
        }
    }
}

impl fmt::Display for ShipError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ShipError {}
