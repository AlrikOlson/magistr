//! Terminal query state and its translation into shared surface capabilities.
//!
//! Cell and window replies retain their precedence here; the shared model
//! receives only resolved measurements, never a query or protocol flag.

use crate::input::Reply;
use magistr_cockpit::cockpit::{Msg, Size};
use magistr_cockpit::pixel::CellSize;

#[derive(Default)]
pub(super) struct TerminalReplies {
    cell: Option<(u16, u16)>,
    window: Option<(u16, u16)>,
}

impl TerminalReplies {
    pub(super) fn new(cell: Option<(u16, u16)>) -> Self {
        Self { cell, window: None }
    }

    pub(super) fn observe(&mut self, reply: Reply, size: Size) -> Msg {
        match reply {
            Reply::Background(color) => return Msg::Background(color),
            Reply::KeyboardFlags(flags) => return Msg::KeyReleases(flags & 2 != 0),
            Reply::CellPixels { width, height } => self.cell = Some((width, height)),
            Reply::WindowPixels { width, height } => self.window = Some((width, height)),
        }
        Msg::CellSize(self.measured(size))
    }

    pub(super) fn measured(&self, size: Size) -> Option<CellSize> {
        measured(self.cell, self.window, size.cols, size.rows)
    }
}

/// The direct CSI 16 t answer wins over the CSI 14 t window fallback,
/// including an invalid direct answer. Preserve the decoder's prior policy.
fn measured(
    cell: Option<(u16, u16)>,
    window: Option<(u16, u16)>,
    cols: u16,
    rows: u16,
) -> Option<CellSize> {
    let (width, height) = cell.or_else(|| {
        let (width, height) = window?;
        (cols > 0 && rows > 0).then(|| (width / cols, height / rows))
    })?;
    (width > 0 && height > 0).then_some(CellSize::new(width, height))
}

#[cfg(test)]
#[path = "terminal_replies.tests.rs"]
mod tests;
