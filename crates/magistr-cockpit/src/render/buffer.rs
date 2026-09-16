//! A grid of cells with wide-grapheme bookkeeping and no out-of-range panics.

use unicode_segmentation::UnicodeSegmentation;

use crate::layout::Rect;

use super::cell::Cell;
use super::style::Style;
use super::width::width;

/// A `width × height` grid of [`Cell`]s in row-major order.
///
/// Writes outside the grid are clipped silently. A grapheme wider than one
/// column is stored as a head cell followed by continuation cells;
/// overwriting either half blanks the other so the invariant "every
/// continuation follows a head that covers it" holds after any sequence of
/// writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Buffer {
    width: u16,
    height: u16,
    cells: Vec<Cell>,
}

impl Buffer {
    /// A blank grid.
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::blank(); usize::from(width) * usize::from(height)],
        }
    }

    /// Columns.
    #[must_use]
    pub fn width(&self) -> u16 {
        self.width
    }

    /// Rows.
    #[must_use]
    pub fn height(&self) -> u16 {
        self.height
    }

    /// Every cell, row-major.
    #[must_use]
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// The cell at `(col, row)`, or `None` outside the grid.
    #[must_use]
    pub fn get(&self, col: u16, row: u16) -> Option<&Cell> {
        self.index(col, row).map(|i| &self.cells[i])
    }

    /// Put one cell at `(col, row)`.
    ///
    /// A wide symbol also claims the following columns as continuations; a
    /// symbol that does not fit before the right edge, a zero-width symbol
    /// and a continuation cell are all stored as a blank in the cell's
    /// style. Outside the grid the write is dropped.
    pub fn set(&mut self, col: u16, row: u16, cell: Cell) {
        let Some(index) = self.index(col, row) else {
            return;
        };
        let columns = cell.width();
        let end = usize::from(col).saturating_add(usize::from(columns));
        if columns == 0 || end > usize::from(self.width) {
            let blank = Cell::new(" ", cell.style);
            self.clear_run(col, row);
            self.cells[index] = blank;
            return;
        }
        for c in col..col.saturating_add(columns) {
            self.clear_run(c, row);
        }
        for c in col.saturating_add(1)..col.saturating_add(columns) {
            if let Some(i) = self.index(c, row) {
                self.cells[i] = Cell::continuation();
            }
        }
        self.cells[index] = cell;
    }

    /// Write `text` starting at `(col, row)` and return the columns used.
    ///
    /// Stops at the right edge; a wide grapheme that would straddle the edge
    /// becomes a blank. Zero-width graphemes (controls, lone combining
    /// marks) are skipped.
    pub fn put_str(&mut self, col: u16, row: u16, text: &str, style: &Style) -> u16 {
        self.put_str_in(
            Rect::new(col, row, self.width.saturating_sub(col), 1),
            text,
            style,
        )
    }

    /// Write on the first row of `rect`, preserving the cells outside it.
    /// A grapheme straddling its right edge becomes a blank inside the clip.
    pub fn put_str_in(&mut self, rect: Rect, text: &str, style: &Style) -> u16 {
        let (col, row) = (rect.x, rect.y);
        let right = rect.right().min(self.width);
        if rect.is_empty() || col >= right || row >= self.height {
            return 0;
        }
        let mut cursor = col;
        for grapheme in text.graphemes(true) {
            let columns = width(grapheme);
            if columns == 0 {
                continue;
            }
            if cursor >= right {
                break;
            }
            if usize::from(cursor) + usize::from(columns) > usize::from(right) {
                self.set(cursor, row, Cell::new(" ", style.clone()));
                cursor = right;
                break;
            }
            self.set(cursor, row, Cell::new(grapheme, style.clone()));
            cursor = cursor.saturating_add(columns);
        }
        cursor - col
    }

    /// Reset every cell to `cell` (a wide `cell` is stored as a blank).
    pub fn fill(&mut self, cell: &Cell) {
        let cell = if cell.width() == 1 {
            cell.clone()
        } else {
            Cell::new(" ", cell.style.clone())
        };
        self.cells.fill(cell);
    }

    /// Replace the grid with a blank one of the new size.
    pub fn resize(&mut self, width: u16, height: u16) {
        *self = Self::new(width, height);
    }

    fn index(&self, col: u16, row: u16) -> Option<usize> {
        (col < self.width && row < self.height)
            .then(|| usize::from(row) * usize::from(self.width) + usize::from(col))
    }

    /// Blank the wide-grapheme run covering `(col, row)`: walk left to its
    /// head, then blank the head and every continuation after it.
    fn clear_run(&mut self, col: u16, row: u16) {
        let Some(mut head) = self.index(col, row) else {
            return;
        };
        let row_start = usize::from(row) * usize::from(self.width);
        while head > row_start && self.cells[head].is_continuation() {
            head -= 1;
        }
        if self.cells[head].width() <= 1
            && !self.cells[head].is_continuation()
            && head == usize::from(col) + row_start
        {
            return;
        }
        self.cells[head] = Cell::blank();
        let row_end = row_start + usize::from(self.width);
        let mut next = head + 1;
        while next < row_end && self.cells[next].is_continuation() {
            self.cells[next] = Cell::blank();
            next += 1;
        }
    }
}

#[cfg(test)]
#[path = "buffer.tests.rs"]
mod tests;
