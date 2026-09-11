/// Descending operation pagination, with inclusive since and exclusive before.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JournalLogQuery {
    pub session_id: Option<String>,
    pub limit: usize,
    pub since_seq: Option<i64>,
    pub before_seq: Option<i64>,
}

impl Default for JournalLogQuery {
    fn default() -> Self {
        Self {
            session_id: None,
            limit: 100,
            since_seq: None,
            before_seq: None,
        }
    }
}
