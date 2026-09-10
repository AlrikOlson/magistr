use alloc::string::String;
use core::fmt;

/// Failure at the host effect boundary. Cancellation does not imply rollback
/// of an effect that the host already admitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectError {
    Denied(String),
    Unavailable(String),
    Cancelled,
    InvalidRequest(String),
    Failed(String),
}

impl fmt::Display for EffectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Denied(reason) => write!(f, "effect denied: {reason}"),
            Self::Unavailable(reason) => write!(f, "effect unavailable: {reason}"),
            Self::Cancelled => f.write_str("effect cancelled"),
            Self::InvalidRequest(reason) => write!(f, "invalid effect request: {reason}"),
            Self::Failed(reason) => write!(f, "effect failed: {reason}"),
        }
    }
}

impl core::error::Error for EffectError {}
