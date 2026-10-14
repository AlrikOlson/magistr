//! What a locked merge-on-save left on disk.

use magistr_workflow_contracts::StoreStamp;

/// The outcome of one locked merge-on-save: the merged state when the file
/// already held one — what actually landed, for the writer's memory to
/// adopt so it answers the store rather than its pre-merge copy — and the
/// stamp of the file as written, taken under the lock so it belongs to
/// exactly those bytes.
#[derive(Debug)]
pub struct MergedWrite<T> {
    /// The merged state, `None` when the writer's own state was written
    /// verbatim because nothing was on disk to merge.
    pub merged: Option<T>,
    /// The stamp of the file after the write; `None` only if the file could
    /// not be stated after the rename, which a caller treats as "unknown,
    /// re-read next time".
    pub stamp: Option<StoreStamp>,
}
