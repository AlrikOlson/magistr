//! One mouse report.

use super::modifiers::Modifiers;
use super::mouse_kind::MouseKind;

/// A mouse report at a cell, with the modifiers held.
///
/// Coordinates are 0-based cells (the wire is 1-based; the decoder
/// shifts them so they index the cell buffer and the hit registry
/// directly).
///
/// A report from a window also carries the pointer's physical pixel
/// ([`MouseEvent::px`]): on a surface whose rows are not all one cell
/// tall the uniform cell division is only approximately where the
/// pointer is, and a drag or a pan that read it would be off by up to
/// a row. The terminal decoder never sets it — a terminal has no pixel
/// to report — so everything that reads the cell reads it unchanged
/// there (see [`Model::pointer_at`](crate::cockpit::Model::pointer_at)).
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
