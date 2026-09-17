use std::collections::HashMap;
use std::sync::RwLock;

use magistr_cockpit::fonts::Faces;
use qs_text::fontdb::{FaceData, FaceInfo, FontDb, FontId, FontStyle, FontWeight};

use crate::display_face::DisplayFace;
use crate::ui_faces::UiFaces;

/// The weight at which a proportional run takes the semibold face.
///
/// qs-ui's own number: `ui/lg` and `ui/xl` are weight 600 and everything
/// below them is 400 or 500, so the family needs exactly one break and
/// exactly two upright faces.
const SEMIBOLD: FontWeight = FontWeight(600);

/// The eight faces this database serves. Indices are the [`FontId`]s.
const FACES: [(&[u8], FontWeight, FontStyle); 8] = [
    (Faces::REGULAR, FontWeight::REGULAR, FontStyle::Normal),
    (Faces::BOLD, FontWeight::BOLD, FontStyle::Normal),
    (Faces::FALLBACK, FontWeight::REGULAR, FontStyle::Normal),
    (Faces::ITALIC, FontWeight::REGULAR, FontStyle::Italic),
    (UiFaces::REGULAR, FontWeight::REGULAR, FontStyle::Normal),
    (UiFaces::SEMIBOLD, SEMIBOLD, FontStyle::Normal),
    (UiFaces::ITALIC, FontWeight::REGULAR, FontStyle::Italic),
    (DisplayFace::REGULAR, FontWeight::REGULAR, FontStyle::Normal),
];

/// The native GUI's font database: the *embedded* faces, never the machine's.
///
/// [`SystemFontDb`](qs_text::SystemFontDb) is the framework's own answer and it is the
/// wrong one here, for the reason the screenshot gate embedded its faces in the first
/// place: a golden rasterized through whatever Segoe UI or Helvetica the host happens
/// to install is not a golden. These are the same three files magistr-tui's
/// `screenshot::Typeface` draws the terminal stills with — Almagest Mono Regular and
/// Bold over a DejaVu Sans Mono fallback — so a native still and a terminal still are
/// drawings of the same outlines and can be compared with each other.
///
/// Three families, not one. The mono family is the *data* voice — every column-aligned
/// row, every id, every diff, engraved with the slashed zero by feature — the
/// proportional family (Hanken Grotesk, [`UiFaces`]) is the UI voice the design
/// language asks for, and the serif ([`DisplayFace`]) is the display voice reserved for
/// report titles and the project cover at ≥20 px. Which of the three a run takes is not
/// a property of the pen; it is the [`Voice`](crate::Voice) the element is set in, or
/// on the cockpit's tree the [`TextRole`](magistr_cockpit::layout::TextRole).
///
/// Deliberately no `query` beyond the three families: a cockpit asks for regular, bold,
/// italic, or whatever covers a symbol, and a database that could answer more would be
/// a database whose answer depends on the machine.
pub struct EmbeddedFonts {
    /// Parsed metadata, resolved lazily — `ttf_parser` is cheap but not
    /// free, and `info` is a diagnostic path rather than a frame one.
    info: RwLock<HashMap<FontId, FaceInfo>>,
}

impl EmbeddedFonts {
    /// The regular face: what a run is shaped with unless it is bold.
    pub const REGULAR: FontId = FontId(0);
    /// The bold face. A *face*, not a synthesized weight, because
    /// the mono family ships one and a smeared outline is not bold.
    pub const BOLD: FontId = FontId(1);
    /// DejaVu Sans Mono, for the symbols neither family has an outline
    /// for. Reached only through [`FontDb::fallback`], per cluster.
    pub const FALLBACK: FontId = FontId(2);
    /// Almagest Mono Italic — the lean the cell rasterizer synthesizes by shearing a
    /// bitmap, which a shaper cannot do.
    pub const MONO_ITALIC: FontId = FontId(3);
    /// Hanken Grotesk Regular: the UI voice at weights 400 and 500.
    pub const UI_REGULAR: FontId = FontId(4);
    /// Hanken Grotesk SemiBold: the UI voice at weight 600, and what
    /// `BOLD` resolves to on a proportional run.
    pub const UI_SEMIBOLD: FontId = FontId(5);
    /// Hanken Grotesk Italic.
    pub const UI_ITALIC: FontId = FontId(6);
    /// Magistr Display Regular: the display voice, worn by
    /// [`Voice::DISPLAY`](crate::Voice::DISPLAY) and nothing else.
    pub const DISPLAY: FontId = FontId(7);

    /// A database over the embedded faces.
    #[must_use]
    pub fn new() -> Self {
        Self {
            info: RwLock::new(HashMap::new()),
        }
    }

    /// Whether `id`'s face has an outline for `ch`.
    fn covers(id: FontId, ch: char) -> bool {
        let Some(data) = face_bytes(id) else {
            return false;
        };
        ttf_parser::Face::parse(data, 0)
            .ok()
            .and_then(|face| face.glyph_index(ch))
            .is_some()
    }
}

impl Default for EmbeddedFonts {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for EmbeddedFonts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbeddedFonts")
            .field("faces", &FACES.len())
            .finish()
    }
}

impl FontDb for EmbeddedFonts {
    fn ui_font(&self) -> FontId {
        Self::UI_REGULAR
    }

    fn ui_font_at(&self, weight: FontWeight) -> FontId {
        // The break is at 600, not at `FontWeight::BOLD` (700): qs-ui's
        // heaviest type role is 600, so a rule keyed on BOLD would send
        // every heading to the regular face and the scale's one weight
        // distinction would never appear on screen.
        if weight >= SEMIBOLD {
            Self::UI_SEMIBOLD
        } else {
            Self::UI_REGULAR
        }
    }

    fn mono_font(&self) -> FontId {
        Self::REGULAR
    }

    fn query(&self, family: &str, weight: FontWeight, style: FontStyle) -> Option<FontId> {
        // Match the embedded family names and the former display name. A miss
        // is `None` rather than the nearest thing, because a caller that
        // asked for a family it did not get should know.
        let family = family.to_ascii_lowercase();
        let italic = style == FontStyle::Italic;
        if family.contains("almagest") {
            Some(match (italic, weight >= FontWeight::BOLD) {
                (true, _) => Self::MONO_ITALIC,
                (false, true) => Self::BOLD,
                (false, false) => Self::REGULAR,
            })
        } else if family.contains("hanken") {
            Some(if italic {
                Self::UI_ITALIC
            } else {
                self.ui_font_at(weight)
            })
        } else if family.contains("magistr display") || family.contains("source serif") {
            // One cut: a weight or a slope the display face does not
            // have is the face it has, never a synthesized one.
            Some(Self::DISPLAY)
        } else if family.contains("dejavu") {
            Some(Self::FALLBACK)
        } else {
            None
        }
    }

    fn fallback(&self, ch: char, preferred: FontId) -> Option<FontId> {
        if Self::covers(preferred, ch) {
            return Some(preferred);
        }
        // One fallback for seven faces. The mono family's bold and
        // italic cover the same set as its regular, and the
        // proportional and display families' gaps are the same
        // box-drawing and geometric shapes the mono family lacks — so
        // the reasoning `Typeface` records holds for all three: a
        // symbol is a shape, not a weight and not a voice.
        Self::covers(Self::FALLBACK, ch).then_some(Self::FALLBACK)
    }

    fn face_data(&self, id: FontId) -> Option<FaceData> {
        face_bytes(id).map(|bytes| FaceData { bytes, index: 0 })
    }

    fn info(&self, id: FontId) -> Option<FaceInfo> {
        if let Some(info) = self.info.read().ok()?.get(&id) {
            return Some(info.clone());
        }
        let (bytes, weight, style) = FACES.get(id.0 as usize).copied()?;
        let parsed = ttf_parser::Face::parse(bytes, 0).ok()?;
        // The first *decodable* record, not the first record: a face can
        // carry its Macintosh (platform 1) name entries ahead of the
        // Windows ones — the bespoke Iosevka build does — and
        // `ttf_parser` only decodes Unicode-capable records, so taking
        // the first match would answer "unknown" for a perfectly named
        // face.
        let named = |id: u16| {
            parsed
                .names()
                .into_iter()
                .filter(move |name| name.name_id == id)
                .find_map(|name| name.to_string())
        };
        let family = named(ttf_parser::name_id::TYPOGRAPHIC_FAMILY)
            .or_else(|| named(ttf_parser::name_id::FAMILY))
            .unwrap_or_else(|| "unknown".to_owned());
        let info = FaceInfo {
            id,
            family,
            weight,
            style,
            monospace: parsed.is_monospaced(),
            path: std::path::PathBuf::new(),
            index: 0,
        };
        self.info.write().ok()?.insert(id, info.clone());
        Some(info)
    }

    fn enumerate(&self) -> Vec<FaceInfo> {
        (0..FACES.len())
            .filter_map(|i| self.info(FontId(i as u32)))
            .collect()
    }
}

/// The bytes behind a [`FontId`], `'static` because they are compiled in
/// — which is exactly the lifetime [`FaceData`] wants and the reason
/// nothing here needs an arena.
fn face_bytes(id: FontId) -> Option<&'static [u8]> {
    FACES.get(id.0 as usize).map(|(bytes, _, _)| *bytes)
}

#[cfg(test)]
#[path = "fonts.tests.rs"]
mod tests;
