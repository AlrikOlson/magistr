//! Damage tracking between the frame on screen and the frame to show.

use super::change::Change;
use magistr_cockpit::render::Buffer;

/// The cells of `next` that differ from `prev`, row-major.
///
/// Continuation cells are never reported: drawing their head repaints
/// them. When the two buffers differ in size every head cell of `next` is
/// reported (the caller clears the screen first).
#[must_use]
pub fn diff<'a>(prev: &Buffer, next: &'a Buffer) -> Vec<Change<'a>> {
    let width = usize::from(next.width());
    let same_size = prev.width() == next.width() && prev.height() == next.height();
    let mut changes = Vec::new();
    if width == 0 {
        return changes;
    }
    for (index, cell) in next.cells().iter().enumerate() {
        if cell.is_continuation() {
            continue;
        }
        if same_size && prev.cells()[index] == *cell {
            continue;
        }
        let (col, row) = (index % width, index / width);
        changes.push(Change {
            col: u16::try_from(col).unwrap_or(u16::MAX),
            row: u16::try_from(row).unwrap_or(u16::MAX),
            cell,
        });
    }
    changes
}

#[cfg(test)]
#[path = "diff.tests.rs"]
mod tests;
