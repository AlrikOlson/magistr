use qs_text::fontdb::FontId;
use qs_text::{Features, PxSize};

/// How one run is set: the face it is shaped with, the size it is
/// shaped at, and the features it is shaped under.
///
/// The three travel together for the reason
/// [`TextEngine`](crate::TextEngine) keeps its shaper, raster and atlas
/// in one object: a run is shaped at a size under its features and its
/// glyphs are rasterized and sampled at *that* size, so a caller holding
/// them apart could shape at one and sample at another — or measure a
/// run with the engraved zero and paint it with the dotted one. It is
/// [`TypeScale`](crate::TypeScale)'s only answer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeSet {
    /// The face.
    pub font: FontId,
    /// The size.
    pub size: PxSize,
    /// The shaping features — the data voice's engraving (`zero`,
    /// `tnum`) or none. A property of the [`Voice`](crate::Voice), not
    /// of the pen.
    pub features: Features,
}
