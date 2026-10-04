use magistr_cockpit::cockpit::{RailMark, Term, TermCell};
use magistr_cockpit::render::{Attrs, Color};
use magistr_cockpit::theme::Icons;
use magistr_theme::ThemeColor;
use qs_gpu::color::Srgba;

use crate::component::Component;
use crate::element::Element;
use crate::lexicon::Lexicon;
use crate::pane_tile::PaneTile;
use crate::voice::Voice;

/// The terminal pane: a running program's screen drawn as cells inside its card — the
/// one component whose colours are the program's and not the design's.
///
/// What it draws: the pane's last [`Term::PANE_ROWS`] rows as a
/// [`Kind::Grid`](crate::element::Kind::Grid) leaf at the data voice's mono grid, each
/// behind the time rail — [`RailMark::COLS`] tiles in the pane's own dim ink reading
/// the row's arrival off its stamp: a tick, a break, a stamp ("Time is a rail") — every
/// colour resolved here — the terminal's default as the text ink on the well, an ANSI
/// index through the medium's own sixteen ([`Lexicon::ansi`]), the 256-colour cube and
/// greys as xterm's, truecolor as itself — and every ink floored against what it sits
/// on at the same ratio the terminal's pen uses ([`Pen::MIN_CONTRAST`]), so a dim grey
/// stays legible and a red stays the program's red. Reverse video is spent on the swap
/// before the floor; dim is the ink drawn halfway toward its ground and then floored,
/// since the face has no dim cut. Bold and italic are the mono family's own cuts.
///
/// The cursor is a filled block in the text ink over the cell's glyph in the ground's
/// colour, blinking on the model's ticks at the caret's own rhythm
/// ([`TermPane::BLINK_STEPS`]) while the program runs and the pane follows; under
/// reduced motion it stands. A thumb along the pane's right edge is sized by the
/// scrollback; while the reader holds the pane off its tail the `↓ Latest · +N` pill
/// floats over the last row with the count. Read-only: nothing here carries an id, so
/// no key can reach the pty through it.
pub struct TermPane;

impl TermPane {
    /// The size the cells are set at — the data voice's, so the pane's
    /// rows sit on the same grid the card's other values do.
    pub const PX: f32 = Voice::VALUE.px;
    /// One period of the cursor's blink, in ticks: about 1.1 s at the runtime's tick —
    /// the caret's own step. The block shows through the first half.
    pub const BLINK_STEPS: u64 = 18;
    /// The pane's own inset inside the card's, and its corner — one
    /// step under the card's 11.
    pub const PAD: f32 = 8.0;
    pub const RADIUS: f32 = 7.0;
    /// The pane's ground: the canvas showing through the card at this
    /// much — a well the output sits in, not a second card.
    pub const WELL: f32 = 0.55;
    /// The thumb's width and its least height.
    pub const THUMB_WIDTH: f32 = 3.0;
    pub const THUMB_MIN: f32 = 6.0;
    /// The air between the cells and the thumb.
    const THUMB_GAP: f32 = 4.0;
    /// The pill's paddings — the chip's, one step tighter.
    const PILL_PAD_X: f32 = 8.0;
    const PILL_PAD_Y: f32 = 2.0;
    /// How far toward its ground a dim cell's ink is drawn before the
    /// floor.
    const DIM: f64 = 0.5;
    /// The rail's tiles ahead of every row's cells.
    pub const RAIL: usize = RailMark::COLS as usize;

    /// The pane for `term` on `ground` (the card's own fill), at tick
    /// `ticks`; `reduced` holds the cursor still.
    #[must_use]
    pub fn el(term: &Term, ticks: u64, reduced: bool, ground: Srgba, lex: &Lexicon) -> Element {
        let cursor = term.cursor().filter(|_| Self::cursor_on(ticks, reduced));
        let rows: Vec<Option<Vec<TermCell>>> =
            (0..Term::PANE_ROWS).map(|row| term.pane_row(row)).collect();
        let marks: Vec<Option<RailMark>> = (0..Term::PANE_ROWS).map(|row| term.mark(row)).collect();
        Self::build(term, &rows, &marks, cursor, ground, lex)
    }

    /// The dock's pane: the terminal's whole window — [`Term::window`] rows, the card's
    /// eight at their tail — cut to `rows` while the dock rises or lowers, in the same
    /// well with the same cursor, thumb and pill.
    #[must_use]
    pub fn window(
        term: &Term,
        rows: usize,
        ticks: u64,
        reduced: bool,
        ground: Srgba,
        lex: &Lexicon,
    ) -> Element {
        let rows = rows.min(term.window());
        // The window's last `rows` — the tail is what a rising dock
        // shows first, the way the card's eight are the window's last.
        let from = term.window() - rows;
        let cursor = term
            .window_cursor()
            .filter(|_| Self::cursor_on(ticks, reduced))
            .and_then(|(row, col)| Some((row.checked_sub(from)?, col)));
        let cells: Vec<Option<Vec<TermCell>>> = (from..term.window())
            .map(|row| term.window_row(row))
            .collect();
        let marks: Vec<Option<RailMark>> = (from..term.window())
            .map(|row| term.window_mark(row))
            .collect();
        Self::build(term, &cells, &marks, cursor, ground, lex)
    }

    /// The pane over `rows` of `term`'s cells (`None` past the content)
    /// behind their rail `marks`, the `cursor` — a `(row, column)` on
    /// those rows — as the block.
    fn build(
        term: &Term,
        rows: &[Option<Vec<TermCell>>],
        marks: &[Option<RailMark>],
        cursor: Option<(usize, u16)>,
        ground: Srgba,
        lex: &Lexicon,
    ) -> Element {
        let well = Self::well_color(ground, lex);
        let dim = Self::dim_ink(well, lex);
        let tiles: Vec<Vec<PaneTile>> = rows
            .iter()
            .enumerate()
            .map(|(row, cells)| {
                let mut cells: Vec<PaneTile> = Self::rail(marks.get(row).copied().flatten(), dim)
                    .chain(
                        cells
                            .as_deref()
                            .unwrap_or_default()
                            .iter()
                            .map(|cell| Self::resolve(cell, well, lex)),
                    )
                    .collect();
                if let Some((r, col)) = cursor
                    && r == row
                    && let Some(cell) = cells.get_mut(Self::RAIL + usize::from(col))
                {
                    // The block: the text ink over the cell, the glyph
                    // in what the cell sat on.
                    cell.ink = cell.ground.unwrap_or(well);
                    cell.ground = Some(lex.tx);
                }
                cells
            })
            .collect();
        let mut body = Element::row()
            .align_start()
            .gap(Self::THUMB_GAP)
            .child(Element::grid(tiles, Self::PX));
        if let Some(track) = Self::thumb(term, rows.len(), lex) {
            body = body.child(track);
        }
        let mut pane = Self::well(ground, lex).child(body);
        if !term.is_following() {
            pane = pane.child(Self::pill(term.arrived(), lex));
        }
        pane
    }

    /// The rail's tiles beside one row: the mark's cells in the pane's
    /// dim ink, blank past the content.
    fn rail(mark: Option<RailMark>, dim: Srgba) -> impl Iterator<Item = PaneTile> {
        let text = mark.map(|mark| mark.text(&Icons::UNICODE));
        (0..Self::RAIL).map(move |col| PaneTile {
            text: text
                .as_ref()
                .and_then(|text| text.chars().nth(col))
                .map_or_else(|| " ".to_owned(), String::from),
            ink: dim,
            ground: None,
            bold: false,
            italic: false,
            underline: false,
        })
    }

    /// The pane's own dim ink: the terminal's default ink dimmed the
    /// way a dim cell is, on `well`.
    fn dim_ink(well: Srgba, lex: &Lexicon) -> Srgba {
        Self::resolve(
            &TermCell {
                text: " ".to_owned(),
                fg: Color::Default,
                bg: Color::Default,
                attrs: Attrs::DIM,
            },
            well,
            lex,
        )
        .ink
    }

    /// Whether the cursor shows at `ticks`: always under reduced
    /// motion, else through the first half of every blink period.
    #[must_use]
    pub fn cursor_on(ticks: u64, reduced: bool) -> bool {
        reduced || ticks % Self::BLINK_STEPS < Self::BLINK_STEPS / 2
    }

    /// The swatch row the kitchen sink exhibits: the sixteen ANSI
    /// grounds, the sixteen inks, and one cell each of the attributes —
    /// bold, italic, underline, reverse, dim — with a truecolor word,
    /// drawn through the same resolution the pane uses, on `ground`.
    #[must_use]
    pub fn swatch(ground: Srgba, lex: &Lexicon) -> Element {
        let mut term = Term::new();
        let grounds: String = (0..16u8)
            .map(|n| format!("\x1b[48;5;{n}m   \x1b[0m"))
            .collect();
        let inks: String = (0..16u8)
            .map(|n| format!("\x1b[38;5;{n}m{n:>3}\x1b[0m"))
            .collect();
        let attrs = "\x1b[1mbold\x1b[0m \x1b[3mitalic\x1b[0m \x1b[4munderline\x1b[0m \
                     \x1b[7mreverse\x1b[0m \x1b[2mdim\x1b[0m \
                     \x1b[38;2;250;179;135mtruecolor\x1b[0m \x1b[38;5;240mgrey 240\x1b[0m";
        term.push(format!("{grounds}\r\n{inks}\r\n{attrs}").as_bytes(), 0);
        term.finish();
        let well = Self::well_color(ground, lex);
        let rows: Vec<Vec<PaneTile>> = (0..3)
            .filter_map(|row| term.pane_row(row))
            .map(|cells| cells.iter().map(|c| Self::resolve(c, well, lex)).collect())
            .collect();
        Self::well(ground, lex).child(Element::grid(rows, Self::PX))
    }

    /// One program cell as the window paints it, on `well` when the
    /// program named no ground.
    #[must_use]
    pub fn resolve(cell: &TermCell, well: Srgba, lex: &Lexicon) -> PaneTile {
        let ink = Self::color(cell.fg, lex).unwrap_or(lex.tx);
        let ground = Self::color(cell.bg, lex);
        let (ink, ground) = if cell.attrs.contains(Attrs::REVERSE) {
            (ground.unwrap_or(well), Some(ink))
        } else {
            (ink, ground)
        };
        let on = ground.unwrap_or(well);
        let ink = if cell.attrs.contains(Attrs::DIM) {
            to_srgba(magistr_theme::math::lerp(
                to_theme(ink),
                to_theme(on),
                Self::DIM,
            ))
        } else {
            ink
        };
        PaneTile {
            text: cell.text.clone(),
            ink: Self::floor(ink, on),
            ground,
            bold: cell.attrs.contains(Attrs::BOLD),
            italic: cell.attrs.contains(Attrs::ITALIC),
            underline: cell.attrs.underline().is_some(),
        }
    }

    /// `ink` held to the terminal pen's floor against `on`.
    #[must_use]
    pub fn floor(ink: Srgba, on: Srgba) -> Srgba {
        Lexicon::readable(ink, on, magistr_cockpit::theme::Pen::MIN_CONTRAST)
    }

    /// A program's colour as the medium's: `None` for the terminal's
    /// default, an ANSI index through the lexicon's sixteen, the rest of
    /// the 256 through xterm's table, truecolor itself.
    fn color(color: Color, lex: &Lexicon) -> Option<Srgba> {
        lex.text_color(color)
    }

    /// The well's colour on `ground`.
    fn well_color(ground: Srgba, lex: &Lexicon) -> Srgba {
        Lexicon::composite(
            ground,
            Srgba::new(lex.base.r, lex.base.g, lex.base.b, Self::WELL),
        )
    }

    /// The well itself: the pane's box on `ground`.
    fn well(ground: Srgba, lex: &Lexicon) -> Element {
        Element::surface()
            .stretch()
            .align_start()
            .pad(Self::PAD)
            .radius(Self::RADIUS)
            .fill(Self::well_color(ground, lex))
    }

    /// The thumb along the pane's right edge, when the program has
    /// written more than the `shown` rows hold: a track as tall as the
    /// cells carrying the window's place in the content as flex shares
    /// — the rows above it, the rows it shows, the rows under it.
    fn thumb(term: &Term, shown: usize, lex: &Lexicon) -> Option<Element> {
        let content = term.content_rows();
        if content <= shown {
            return None;
        }
        let below = term.offset();
        let shown = shown.min(content);
        let above = content.saturating_sub(below + shown);
        #[allow(clippy::cast_precision_loss)]
        let share = |rows: usize| rows as f32;
        Some(
            Element::surface()
                .width(Self::THUMB_WIDTH)
                .stretch()
                .child(Element::surface().grow(share(above)).min_height(0.0))
                .child(
                    Element::surface()
                        .grow(share(shown))
                        .min_height(Self::THUMB_MIN)
                        .radius(Self::THUMB_WIDTH / 2.0)
                        .fill(lex.ov0),
                )
                .child(Element::surface().grow(share(below)).min_height(0.0)),
        )
    }

    /// The jump pill, floating over the pane's foot at its right edge:
    /// `↓ Latest · +N`, the count of rows that arrived under a held
    /// window.
    fn pill(arrived: usize, lex: &Lexicon) -> Element {
        Element::row()
            .pad_x(Self::PILL_PAD_X)
            .pad_y(Self::PILL_PAD_Y)
            .radius(999.0)
            .fill(lex.s1)
            .pin_right(Self::PAD)
            .pin_bottom(Self::PAD)
            .child(
                Element::text(
                    Voice::CHIP,
                    lex.tx,
                    format!("{} Latest \u{b7} +{arrived}", Icons::UNICODE.latest),
                )
                .claim(Component::Chip)
                .nowrap(),
            )
    }
}

/// A palette colour as the window's.
fn to_srgba(color: ThemeColor) -> Srgba {
    Srgba::new(
        f32::from(color.r) / 255.0,
        f32::from(color.g) / 255.0,
        f32::from(color.b) / 255.0,
        1.0,
    )
}

/// The window's colour as a palette colour, its alpha dropped.
fn to_theme(color: Srgba) -> ThemeColor {
    let byte = |v: f32| {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let b = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        b
    };
    ThemeColor::new(byte(color.r), byte(color.g), byte(color.b))
}

#[cfg(test)]
#[path = "term_pane.tests.rs"]
mod tests;
