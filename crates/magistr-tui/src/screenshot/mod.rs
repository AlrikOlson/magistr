//! The screenshot gate: a cockpit frame as the pixels a
//! terminal would show, so visual regressions are judged on what a person
//! sees — colours, weight, chrome — not on the attr-blind text dumps the
//! insta snapshots are.
//!
//! [`frame`] runs the real view, layout and painter into a cell
//! [`Buffer`](magistr_cockpit::render::Buffer) and [`rasterize`] turns that buffer
//! into an opaque RGBA pixmap: every cell's background from the theme's
//! palette, its grapheme drawn with the embedded [`Typeface`] (JetBrains
//! Mono, bold as its own face, italic as a shear, and a DejaVu Sans Mono
//! fallback for the symbols JetBrains Mono has no outline for), dim as a
//! blend toward the background, reverse as a swap, underline and
//! strikethrough as rules.
//! Nothing platform-specific is involved — no system font, no terminal —
//! so the same model renders the same bytes on every machine, which is
//! what lets [`Golden`] compare a frame against a checked-in PNG.

mod boxdraw;
mod frame;
mod glyph;
mod kitty;
mod placement;
mod profile;
mod raster;
mod strip;
mod typeface;

pub use frame::frame;
pub use glyph::Glyph;
pub use placement::Placement;
pub use profile::Profile;
pub use raster::rasterize;
pub use strip::strip;
pub use typeface::Typeface;
