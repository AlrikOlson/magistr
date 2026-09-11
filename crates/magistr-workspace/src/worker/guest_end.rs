use serde::{Deserialize, Serialize};

/// Guest termination remains ordinary typed data, including resource limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "kebab-case")]
pub enum WorkerGuestEnd {
    Ok,
    OutOfFuel,
    MemoryCap,
    CapDenied(String),
    GuestTrap(String),
    GuestExit(i32),
    InvalidGuest(String),
}

impl From<crate::sandbox::GuestEnd> for WorkerGuestEnd {
    fn from(end: crate::sandbox::GuestEnd) -> Self {
        use crate::sandbox::GuestEnd;
        match end {
            GuestEnd::Ok => Self::Ok,
            GuestEnd::OutOfFuel => Self::OutOfFuel,
            GuestEnd::MemoryCap => Self::MemoryCap,
            GuestEnd::CapDenied(detail) => Self::CapDenied(detail),
            GuestEnd::GuestTrap(detail) => Self::GuestTrap(detail),
            GuestEnd::GuestExit(code) => Self::GuestExit(code),
            GuestEnd::InvalidGuest(detail) => Self::InvalidGuest(detail),
        }
    }
}
