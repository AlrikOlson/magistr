//! The screenshot gate: a cockpit frame as the pixels a
//! terminal would show, so visual regressions are judged on what a person
//! sees — colours, weight, chrome — not on the attr-blind text dumps the
//! insta snapshots are.
//!
//! [`frame`] uses the runtime's actual layout and drawing path. Graphics
//! profiles decode the transmitted opaque strips, including Source Sans 3
//! typography, vector marks and images. The cell profile rasterizes its
//! [`Buffer`](magistr_cockpit::render::Buffer) with the embedded [`Typeface`].
//! Nothing platform-specific is involved — no system font, no terminal —
//! so the same model renders the same bytes on every machine, which is
//! what lets [`Golden`] compare a frame against a checked-in PNG.

mod boxdraw;
mod frame;
mod glyph;
mod kitty;
mod placement;
mod preview;
mod profile;
mod raster;
mod strip;
mod typeface;

pub use frame::{frame, layout};
pub use glyph::Glyph;
pub(crate) use kitty::decode;
pub use placement::Placement;
pub use preview::Preview;
pub use profile::Profile;
pub use raster::rasterize;
pub use strip::strip;
pub use typeface::Typeface;
