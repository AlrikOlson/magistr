/// Descending eligible operations, optionally narrowed by session and sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UndoCandidateQuery {
    pub session_id: Option<String>,
    pub limit: usize,
    pub seqs: Option<Vec<i64>>,
}

impl Default for UndoCandidateQuery {
    fn default() -> Self {
        Self {
            session_id: None,
            limit: 100,
            seqs: None,
        }
    }
}
