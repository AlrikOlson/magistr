//! The embedded monospace faces, as bytes.
//!
//! One copy for every renderer magistr has. The terminal renderer's
//! screenshot gate (magistr-tui's `screenshot::Typeface`) parses these
//! with `fontdue`; the native GUI parses the same bytes with `swash` and
//! `rustybuzz` through its own `FontDb`. That they are the *same* bytes
//! is what lets a native still be compared with a terminal still at all,
//! and it is why the licences below cover both.

/// Almagest Mono (SIL Open Font License 1.1 — `fonts/OFL.txt`), the
/// cockpit's bespoke face: an Iosevka custom build compiled from
/// `fonts/almagest-mono.toml` by `scripts/build-mono-face.sh`.
///
/// The build is metric-pinned to the cell contract JetBrains Mono set —
/// upem 1000, advance 600 (0.6 em), hhea 1020/−300/0 — so swapping the
/// face moved no cell, wrap point or grid anywhere; only outlines
/// changed.
pub struct Faces;

impl Faces {
    /// Almagest Mono Regular.
    pub const REGULAR: &'static [u8] = include_bytes!("../fonts/AlmagestMono-Regular.ttf");
    /// Almagest Mono Bold, the same licence.
    pub const BOLD: &'static [u8] = include_bytes!("../fonts/AlmagestMono-Bold.ttf");
    /// Almagest Mono Italic, the same licence.
    ///
    /// Read only by the native GUI. The cell rasterizer synthesizes its
    /// lean by shearing the regular face's bitmap after it is drawn — a
    /// post-rasterization trick a shaper has no equivalent for, which is
    /// why the second renderer needs the real outlines.
    /// One italic and no bold-italic: the cockpit's pens never combine
    /// the two, so a face for that pairing would be a face nothing asks
    /// for.
    pub const ITALIC: &'static [u8] = include_bytes!("../fonts/AlmagestMono-Italic.ttf");
    /// DejaVu Sans Mono (Bitstream Vera + Arev licences —
    /// `fonts/DejaVu-LICENSE.txt`), the fallback for the symbols
    /// the primary face has no outline for. One face and not
    /// two — a symbol is a shape rather than a weight, so bold asks it
    /// for the same glyph.
    pub const FALLBACK: &'static [u8] = include_bytes!("../fonts/DejaVuSansMono.ttf");
}
