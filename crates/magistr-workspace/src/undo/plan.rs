use std::collections::{BTreeMap, BTreeSet};

use crate::file_content::FileContent;
use crate::journal::{JournalFileKind, JournalOp};

use super::{UndoConflict, UndoConflictPolicy, UndoPlanError, UndoSkippedFile, UndoWrite};

/// A proposed restoration, never proof that files were written or history consumed.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UndoPlan {
    pub writes: Vec<UndoWrite>,
    /// May be recorded only after the later mutation pipeline succeeds.
    pub undo_of: Vec<i64>,
    pub conflicts: Vec<UndoConflict>,
    pub evicted: Vec<UndoSkippedFile>,
    pub kept: Vec<UndoSkippedFile>,
    pub notes: Vec<String>,
}

impl UndoPlan {
    /// Candidates must come from the journal's session-aware eligibility query.
    /// Snapshots must be read through workspace admission; absent paths need an
    /// explicit None. Application must revalidate existence and hashes, preserve
    /// exact bytes, and disable formatting.
    pub fn build(
        candidates: &[JournalOp],
        current: &BTreeMap<String, Option<FileContent>>,
        policy: UndoConflictPolicy,
    ) -> Result<Self, UndoPlanError> {
        if candidates.len() > 100
            || candidates.iter().any(|op| {
                let mut paths = BTreeSet::new();
                op.seq <= 0
                    || op.undo_of.is_some()
                    || op.files.iter().any(|file| !paths.insert(&file.path))
            })
            || candidates.windows(2).any(|pair| pair[0].seq <= pair[1].seq)
        {
            return Err(UndoPlanError::InvalidCandidates);
        }
        let mut plan = Self::default();
        // Borrow snapshots until producing the final owned write batch. Insertion
        // order follows the journal's file ordering, not lexical path order.
        let mut simulated = BTreeMap::new();
        let mut order = Vec::new();
        for op in candidates {
            if has_evicted_blob(op) {
                plan.evicted
                    .extend(op.files.iter().map(|file| UndoSkippedFile {
                        path: file.path.clone(),
                        op_seq: op.seq,
                    }));
                plan.notes.push(format!(
                    "undo: op {} has unavailable journal blobs; skipped the whole operation and preserved its history",
                    op.seq
                ));
                continue;
            }
            for file in &op.files {
                let path = file.path.as_str();
                let observed = current
                    .get(path)
                    .ok_or_else(|| UndoPlanError::MissingSnapshot(path.to_owned()))?
                    .as_ref();
                let value = if let Some(value) = simulated.get(path) {
                    *value
                } else {
                    order.push(path);
                    simulated.insert(path, observed);
                    observed
                };
                let expected = if file.kind == JournalFileKind::Delete {
                    None
                } else {
                    file.after.as_ref()
                };
                let target = if file.kind == JournalFileKind::Create {
                    None
                } else {
                    file.before.as_ref()
                };
                if value != expected {
                    match policy {
                        UndoConflictPolicy::Surface => {
                            plan.conflicts.push(UndoConflict {
                                path: path.to_owned(),
                                op_seq: op.seq,
                                expected_hash: expected.map(|_| file.after_hash.clone()),
                                current_hash: value.map(FileContent::content_hash),
                                restore_hash: target.map(|_| file.before_hash.clone()),
                            });
                            continue;
                        }
                        UndoConflictPolicy::KeepCurrent => {
                            plan.kept.push(UndoSkippedFile {
                                path: path.to_owned(),
                                op_seq: op.seq,
                            });
                            continue;
                        }
                        UndoConflictPolicy::Force => {
                            plan.notes.push(format!(
                                "undo: {path} changed after op {}; force proposes restoring the journaled bytes",
                                op.seq
                            ));
                        }
                    }
                }
                simulated.insert(path, target);
            }
            plan.undo_of.push(op.seq);
        }
        if !plan.conflicts.is_empty() {
            plan.undo_of.clear();
            plan.notes.push(
                "undo: conflicts block the complete restoration; no writes or consumed operations are proposed".into(),
            );
            return Ok(plan);
        }
        for path in order {
            let target = simulated[path];
            let observed = current[path].as_ref();
            if target == observed {
                continue;
            }
            plan.writes.push(UndoWrite {
                path: path.to_owned(),
                content: target.cloned(),
                expected_hash: observed.map_or_else(
                    || FileContent::Text(String::new()).content_hash(),
                    FileContent::content_hash,
                ),
                expected_exists: observed.is_some(),
            });
        }
        Ok(plan)
    }
}

fn has_evicted_blob(op: &JournalOp) -> bool {
    op.files.iter().any(|file| {
        (file.kind != JournalFileKind::Delete && file.after.is_none())
            || (file.kind != JournalFileKind::Create && file.before.is_none())
    })
}

#[cfg(test)]
#[path = "plan.tests.rs"]
mod tests;
