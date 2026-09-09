//! Golden images: the review flow that compares a rasterized frame
//! against its checked-in PNG. One comparer for both golden homes — the
//! terminal stills and filmstrips under `crates/magistr-tui/tests/golden`
//! and the native stills under `crates/magistr-gui2/tests/golden` — so
//! the two renderers are judged by the same rule: decoded pixels, never
//! PNG bytes, and no unreviewed candidate passes.

pub mod directory;
pub mod error;

pub use directory::Golden;
pub use error::GoldenError;
