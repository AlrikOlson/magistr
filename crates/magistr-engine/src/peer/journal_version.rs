use serde::{Deserialize, Serialize};

use super::AgreementError;

/// Replay must retain a run's original limit semantics, including recorded stops.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub(super) enum JournalVersion {
    LegacyTerminal,
    StartCapacity,
    SharedBriefing,
}

impl JournalVersion {
    pub(super) fn stops_for_attempts(self, observed: u32, ceiling: u32) -> bool {
        match self {
            Self::LegacyTerminal => observed >= ceiling,
            Self::StartCapacity | Self::SharedBriefing => observed > ceiling,
        }
    }
}

impl TryFrom<u32> for JournalVersion {
    type Error = AgreementError;

    fn try_from(version: u32) -> Result<Self, Self::Error> {
        match version {
            1 => Ok(Self::LegacyTerminal),
            2 => Ok(Self::StartCapacity),
            3 => Ok(Self::SharedBriefing),
            _ => Err(AgreementError::Invalid(
                "unsupported peer journal version".into(),
            )),
        }
    }
}

impl From<JournalVersion> for u32 {
    fn from(version: JournalVersion) -> Self {
        match version {
            JournalVersion::LegacyTerminal => 1,
            JournalVersion::StartCapacity => 2,
            JournalVersion::SharedBriefing => 3,
        }
    }
}
