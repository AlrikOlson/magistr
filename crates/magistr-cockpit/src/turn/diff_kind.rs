//! What a line of a unified diff is, for the pen.

use super::row_kind::RowKind;

/// The three markers `git` leaves in a file it could not merge.
const CONFLICT_MARKERS: [&str; 3] = ["<<<<<<<", "=======", ">>>>>>>"];

/// The [`RowKind`] a diff line is painted as: a conflict marker first —
/// it can arrive as an added, a removed or a context line, so its own
/// `+`/`-`/space is stripped before the marker is looked for — then
/// added, removed, a hunk header, and context for everything else.
///
/// One classifier, so the Changes tab's expanded journal entry and the
/// turn feed's edit card never disagree about what a line means.
#[must_use]
pub fn diff_kind(line: &str) -> RowKind {
    let body = line.strip_prefix(['+', '-', ' ']).unwrap_or(line);
    if CONFLICT_MARKERS.iter().any(|mark| body.starts_with(mark)) {
        return RowKind::Conflict;
    }
    match line.as_bytes().first() {
        Some(b'+') => RowKind::DiffAdd,
        Some(b'-') => RowKind::DiffRemove,
        Some(b'@') => RowKind::Fence,
        _ => RowKind::Code,
    }
}

#[cfg(test)]
#[path = "diff_kind.tests.rs"]
mod tests;
