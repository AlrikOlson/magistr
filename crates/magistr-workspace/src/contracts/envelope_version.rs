use super::{ErrorKind, WorkspaceError};

/// A negotiated, supported response version. Negotiation is per request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnvelopeVersion(u64);

impl EnvelopeVersion {
    pub const MIN: Self = Self(1);
    pub const LATEST: Self = Self(1);

    pub fn negotiate(maximum: Option<u64>) -> Result<Self, WorkspaceError> {
        match maximum {
            None => Ok(Self::LATEST),
            Some(0) => Err(WorkspaceError::new(
                ErrorKind::UnsupportedVersion,
                "workbench/maxEnvelopeVersion must be a positive integer, got 0",
            )),
            Some(maximum) if maximum < Self::MIN.0 => Err(WorkspaceError::new(
                ErrorKind::UnsupportedVersion,
                format!(
                    "client maximum envelope version {maximum} is below the server minimum {}",
                    Self::MIN.0
                ),
            )),
            Some(maximum) => Ok(Self(maximum.min(Self::LATEST.0))),
        }
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}
