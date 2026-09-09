use serde::{Deserialize, Serialize};

use super::batch_state::BatchState;

/// One project's row in a batch rollup.
///
/// `job_id` is *derived* from the batch id and the project id rather
/// than remembered, so the row and the [`WorkQueue`] entry it names can
/// never come apart — there is no map for them to drift through, and a
/// project that has not been released yet still names the job it will
/// be released as.
///
/// [`WorkQueue`]: crate::fleet::WorkQueue
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchRow {
    /// The project id.
    pub project: String,
    /// Where it stands.
    pub state: BatchState,
    /// The job that carries it, released or not.
    pub job_id: String,
}
