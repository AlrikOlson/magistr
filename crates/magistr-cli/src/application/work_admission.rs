use magistr_domain::identity::RunId;

/// The owner's actual launch result, retained independently of the transcript.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct WorkAdmission {
    pub sequence: u64,
    pub run_id: Option<RunId>,
    pub error: Option<String>,
}
