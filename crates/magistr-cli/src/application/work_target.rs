use magistr_domain::identity::{RunId, SessionAddress};

/// The exact running work within a resident session, independent of conversation changes.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WorkTarget {
    pub address: SessionAddress,
    pub run_id: RunId,
}
