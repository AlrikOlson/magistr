//! The serif display family, as bytes.
//!
//! Beside [`UiFaces`](crate::ui_faces::UiFaces) and not inside it: the
//! sans is the UI voice and appears on every chrome word, the serif is
//! the *display* voice and appears on exactly one kind of thing. Keeping
//! them in separate files keeps the rule readable at the point the
//! bytes are declared.

/// Magistr Display Regular, derived from Adobe's Source Serif 4 Display (SIL Open Font
/// License 1.1 — `fonts/SourceSerif4-OFL.txt`), the serif reserves for brand moments at
/// ≥20 px — report titles and the project cover — and nowhere else. The volume's own
/// stack is Iowan Old Style / Palatino / Charter / Georgia, a sturdy book serif none of
/// which can be embedded; Source Serif 4 is that register's open neighbour, and its
/// *Display* optical size is drawn for exactly the sizes the design allows it.
///
/// One cut, tailored: the release's static Display Regular (4.005R) subset with
/// fonttools to Latin-1, Latin Extended-A and the punctuation, currency and arrow
/// blocks a title can carry, keeping `kern`, `liga`, `ccmp`, `locl`, `mark` and `mkmk`
/// — 297 KB to 71 KB, 427 glyphs. Its internal family and PostScript names are Magistr
/// Display because the subset is a modified font; the original copyright and OFL
/// metadata remain. See `fonts/README.md` for the naming change. No italic and no
/// second weight, because a display face with one home needs one voice; a title wanting
/// emphasis is a design question, not a face the crate should silently have. Embedded,
/// not system, for the reason the screenshot gate embedded its faces in the first
/// place: a golden rasterized through whatever serif the host installs is not a golden.
pub struct DisplayFace;

impl DisplayFace {
    /// Magistr Display Regular; the existing asset path is retained.
    pub const REGULAR: &'static [u8] = include_bytes!("../fonts/SourceSerif4Display-Regular.ttf");
}
