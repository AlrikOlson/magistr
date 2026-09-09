use qs_gpu::color::Srgba;

/// One cell of a program's terminal as the window paints it: the
/// grapheme, its ink already resolved through the medium's lexicon and
/// floored against what it sits on, the ground the program named (or
/// none, when the pane's own well shows through), and the cuts and rule
/// its attributes ask for.
///
/// Resolution happens once, in [`TermPane`](crate::TermPane): the
/// terminal's default, an ANSI index, the xterm cube and truecolor all
/// become one `Srgba` there, reverse video is spent on the swap there,
/// and the contrast floor is applied there — so the emit walk that
/// draws a [`Kind::Grid`](crate::element::Kind::Grid) leaf is a
/// painter with no opinion about colour, the way it is for every other
/// kind.
///
/// A wide grapheme's second column is a cell with no text at all.
#[derive(Clone, Debug, PartialEq)]
pub struct PaneTile {
    /// The grapheme, empty for the continuation of a wide one.
    pub text: String,
    /// The ink the glyph is drawn in.
    pub ink: Srgba,
    /// The ground under the cell, when the program named one.
    pub ground: Option<Srgba>,
    /// Drawn on the mono face's bold cut.
    pub bold: bool,
    /// Drawn on the mono face's italic cut.
    pub italic: bool,
    /// A hairline under the cell in its ink.
    pub underline: bool,
}

impl PaneTile {
    /// Whether the cell shows nothing of the program's — no grapheme,
    /// or the tail of a wide one.
    #[must_use]
    pub fn is_blank(&self) -> bool {
        self.text.is_empty()
    }

    /// Whether the cell paints nothing at all: a plain space on the
    /// well with no rule under it — the state of every cell the
    /// program never wrote to.
    #[must_use]
    pub fn is_quiet(&self) -> bool {
        (self.text == " " || self.text.is_empty()) && self.ground.is_none() && !self.underline
    }
}
