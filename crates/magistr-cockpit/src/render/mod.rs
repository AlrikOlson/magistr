//! The cell vocabulary and the cell painter: a styled grapheme, a grid of
//! them, a run of scaled text, and `paint` putting a laid-out view into
//! the grid.
//!
//! What happens to the grid next is the renderer's: magistr-tui diffs it
//! against the last frame and encodes the difference for a terminal;
//! magistr-gui2 never fills one and walks the same tree into a draw list
//! instead. [`resolve`] is the one colour table both of them read a cell
//! colour through, which is what lets a native still and a terminal
//! still be compared at all.

pub mod attrs;
pub mod buffer;
pub mod cell;
pub mod color;
pub mod headline;
pub mod paint;
pub mod resolve;
pub mod style;
pub mod underline;
pub mod width;

pub use attrs::Attrs;
pub use buffer::Buffer;
pub use cell::Cell;
pub use color::Color;
pub use headline::Headline;
pub use paint::paint;
pub use resolve::resolve;
pub use style::Style;
pub use underline::Underline;
pub use width::width;
