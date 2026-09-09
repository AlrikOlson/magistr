//! What the surface under the cockpit can show, as the core hears it.
//!
//! The core never learns *which* terminal it is on or *which* protocol
//! pixels travel by. It learns one value, once, at startup: a [`Mode`] —
//! pixel widgets render or everything is cells, whether the surface takes
//! the pointer-shape request, and, when there are no pixels, the one-line
//! [`TextReason`] the user is told. The terminal runtime translates its
//! probe's verdict into a `Mode`; the native window declares one
//! directly. `Protocol`, `Passthrough` and `Kitty` live in magistr-tui
//! and never cross this seam — `tests/boundary.rs` refuses them here.

pub mod mode;
pub mod text_reason;

pub use mode::Mode;
pub use text_reason::TextReason;
