//! One mouse report.

use super::modifiers::Modifiers;
use super::mouse_kind::MouseKind;

/// A mouse report at a cell, with the modifiers held.
///
/// Coordinates are 0-based cells (the wire is 1-based; the decoder
/// shifts them so they index the cell buffer and the hit registry
/// directly).
///
/// Surfaces with physical pointer reports also carry the exact pixel
/// ([`MouseEvent::px`]). The runtime translates physical reports into the
/// current cell coordinates before dispatch, keeping both representations.
/// A surface that reports only cells leaves `px` absent (see
/// [`Model::pointer_at`](crate::cockpit::Model::pointer_at)).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MouseEvent {
    /// What happened.
    pub kind: MouseKind,
    /// Column, 0-based.
    pub col: u16,
    /// Row, 0-based.
    pub row: u16,
    /// Modifier keys held.
    pub mods: Modifiers,
    /// The pointer in physical pixels, when the surface has them.
    pub px: Option<(u32, u32)>,
}

impl MouseEvent {
    /// A report of `kind` at `(col, row)` with no modifiers and no
    /// pixel — what a terminal reports.
    #[must_use]
    pub const fn new(kind: MouseKind, col: u16, row: u16) -> Self {
        Self {
            kind,
            col,
            row,
            mods: Modifiers::NONE,
            px: None,
        }
    }

    /// The same report with `mods` held.
    #[must_use]
    pub const fn with(mut self, mods: Modifiers) -> Self {
        self.mods = mods;
        self
    }

    /// The same report with the pointer's physical pixel — what a
    /// window adds to the cell it already resolved.
    #[must_use]
    pub const fn at_px(mut self, x: u32, y: u32) -> Self {
        self.px = Some((x, y));
        self
    }
}

#[cfg(test)]
#[path = "mouse_event.tests.rs"]
mod tests;
