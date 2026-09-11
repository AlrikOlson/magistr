use serde::Serialize;

use super::WriteReport;

/// One undo answer row: an applied restoration, or a file deliberately left alone.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UndoRow {
    Write(WriteReport),
    /// A later change touched the file: the three-way choice, as data.
    /// Absent content is rendered as the empty hash string, as retained.
    #[serde(rename_all = "camelCase")]
    Conflict {
        path: String,
        op_seq: i64,
        expected_hash: String,
        current_hash: String,
        restore_hash: String,
    },
    #[serde(rename_all = "camelCase")]
    Evicted {
        path: String,
        op_seq: i64,
    },
    #[serde(rename_all = "camelCase")]
    Kept {
        path: String,
        op_seq: i64,
    },
}
