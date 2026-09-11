use serde::Deserialize;

use crate::commands::UndoScope;

/// The read half of the undo tool (`action: "list"`): the journal as rows,
/// newest op first, writing nothing. Reading another session's history
/// reverts nothing, so the scope defaults to the whole workspace here where
/// a rewind defaults to the calling session; the budget default is the
/// retained wider one, because a diff-carrying row is large.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct JournalListInput {
    pub scope: UndoScope,
    /// How many ops to return, newest first.
    pub limit: usize,
    /// Render each row's unified diff; false keeps the counts and drops the bodies.
    pub diff: bool,
    /// Only ops with `seq >= sinceSeq`: tailing since the last one seen.
    pub since_seq: Option<i64>,
    /// Only ops with `seq < beforeSeq`: the pagination cursor a cut hands back.
    pub before_seq: Option<i64>,
    pub budget: u64,
}

impl JournalListInput {
    pub const LIMIT_DEFAULT: usize = 20;
    pub const LIMIT_MAX: usize = 200;
    pub const BUDGET_DEFAULT: u64 = 8000;
}

impl Default for JournalListInput {
    fn default() -> Self {
        Self {
            scope: UndoScope::Workspace,
            limit: Self::LIMIT_DEFAULT,
            diff: true,
            since_seq: None,
            before_seq: None,
            budget: Self::BUDGET_DEFAULT,
        }
    }
}
