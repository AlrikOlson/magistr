//! The terminal's side of the cell renderer: a double-buffered pair of
//! the cockpit's cell grids ([`Buffer`](magistr_cockpit::render::Buffer)),
//! a damage diff between frames, and an encoder that turns the diff into
//! the fewest bytes the terminal needs.
//!
//! Draw into [`Screen::buffer`], call [`Screen::present`], and the byte
//! stream for exactly that frame lands in your `Vec<u8>` bracketed by one
//! BSU/ESU pair.

pub mod change;
pub mod diff;
pub mod encode;
mod pixel_frame;
pub(crate) mod pixel_type;
pub(crate) mod pixel_view;
pub mod screen;

pub use change::Change;
pub use diff::diff;
pub use encode::Encoder;
pub use screen::Screen;
