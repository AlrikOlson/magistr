use magistr_cockpit::cockpit::{Model, Search, SearchHit, SearchKind, SearchScope};
use qs_gpu::color::Srgba;

use crate::card::Card;
use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::icon::Icon;
use crate::lexicon::Lexicon;
use crate::mark::Mark;
use crate::match_line::MatchLine;
use crate::medium::Medium;
use crate::scope_chip::ScopeChip;
use crate::search_row::SearchRow;
use crate::voice::Voice;

/// The ⌘⇧F search: the volume's view 13 driven by the model — the glass card at the top
/// of the window, the field row (the search glyph, the chord, the query and its caret,
/// the live count), the scope chips, and the hits grouped under the book's own caps:
/// reports · trace · receipts · sessions.
///
/// A true overlay: the composition root dims the room behind it and floats this over
/// the dimmed room; the composition carries its own pin and seat. Motion: the caret
/// only — search is a decision screen.
pub struct SearchComposition;

impl SearchComposition {
    /// The result rows' element ids: `ROWS + index` for the row at
    /// `index` in [`SearchResults::rows`](magistr_cockpit::cockpit::SearchResults::rows),
    /// `ROWS_END` exclusive — its own indexed space past the Signals'.
    pub const ROWS: u64 = 1 << 26;
    pub const FIELD: ElementId = ElementId(434);
    /// One indexed space's width — far more rows than search cuts to.
    pub const ROWS_END: u64 = Self::ROWS + 64;
    /// The scope chips' element ids: `SCOPES + index` for the chip at
    /// `index` in [`SearchScope::ALL`], `SCOPES_END` exclusive.
    pub const SCOPES: u64 = 1 << 27;
    /// The end of the chip space.
    pub const SCOPES_END: u64 = Self::SCOPES + SearchScope::ALL.len() as u64;

    /// The card's measure and seat in the capture: view 13's float is
    /// 600px wide with its top edge 52px under the window's.
    const WIDTH: f32 = 600.0;
    const TOP: f32 = 52.0;
    /// The card's own padding (`.ov7`'s `padding:6px 8px`).
    const PAD_X: f32 = 8.0;
    const PAD_Y: f32 = 6.0;
    /// The field row's and the chip row's padding (`padding:9px 12px`)
    /// and gaps (`gap:10px`, `gap:6px`).
    const BAND_PAD_X: f32 = 12.0;
    const BAND_PAD_Y: f32 = 9.0;
    const FIELD_GAP: f32 = 10.0;
    const CHIPS_GAP: f32 = 6.0;
    /// The search glyph (`.ic` 13px) and the chord's mono 11.
    const ICON_PX: u32 = 13;
    const CHORD: Voice = Voice {
        px: 11.0,
        ..Voice::CHIP
    };
    /// The query's voice — the field's own sans 13.5.
    const QUERY: Voice = Voice {
        px: 13.5,
        ..Voice::STEER
    };
    /// The caret: the accent at the query's writing edge.
    const CARET: (f32, f32) = (1.5, 14.0);
    /// The results block (`padding:10px 12px 4px`).
    const RESULTS_PAD_TOP: f32 = 10.0;
    const RESULTS_PAD_X: f32 = 12.0;
    const RESULTS_PAD_BOTTOM: f32 = 4.0;
    /// A group cap: `.cap7` with its 11px glyph and gap, and the air
    /// under it (`margin-bottom:6px`).
    const CAP_ICON_PX: u32 = 11;
    const CAP_GAP: f32 = 4.0;
    const CAP_BELOW: f32 = 6.0;
    /// The air under a row — four under the chosen one, six under the
    /// rest — and the last row's deeper foot (`padding:7px 10px 10px`).
    const ROW_BELOW_CHOSEN: f32 = 4.0;
    const ROW_BELOW: f32 = 6.0;
    const ROW_PAD_BOTTOM: f32 = 7.0;
    const LAST_ROW_PAD_BOTTOM: f32 = 10.0;

    /// The search float for `model`, pinned over the whole window —
    /// `None` while the search is down.
    #[must_use]
    pub fn el(model: &Model, medium: Medium) -> Option<Element> {
        let search = model.search()?;
        let lex = Lexicon::of(medium);
        let glass = Lexicon::composite(
            lex.base,
            Srgba::new(lex.mantle.r, lex.mantle.g, lex.mantle.b, 0.84),
        );
        let results = Search::results(model);
        let card = Card::popover(lex.base, Self::PAD_X, Self::PAD_Y, &lex)
            .width(Self::WIDTH)
            .child(field(search, &results.count_text(), glass, &lex))
            .child(chips(search, glass, &lex))
            .child(rows(search, &results.rows, glass, &lex));
        Some(
            Element::surface()
                .pin_left(0.0)
                .pin_top(0.0)
                .pin_right(0.0)
                .pin_bottom(0.0)
                .center()
                .child(Element::surface().height(Self::TOP))
                .child(card),
        )
    }
}

/// The field band: the glyph, the chord, what has been typed with its
/// caret, and the count at the far edge — over the band's hairline.
fn field(search: &Search, count: &str, glass: Srgba, lex: &Lexicon) -> Element {
    Element::surface()
        .child(
            Element::row()
                .pad_x(SearchComposition::BAND_PAD_X)
                .pad_y(SearchComposition::BAND_PAD_Y)
                .gap(SearchComposition::FIELD_GAP)
                .center()
                .child(Element::mark(
                    Mark::Icon(Icon::Search),
                    SearchComposition::ICON_PX,
                    Lexicon::ink_bytes(lex.mauve),
                ))
                .child(
                    Element::text(SearchComposition::CHORD, lex.mauve, "\u{2318}\u{21e7}F")
                        .claim(Component::KeyHint)
                        .nowrap(),
                )
                .child(
                    Element::row()
                        .grow(1.0)
                        .center()
                        .child(
                            Element::text(SearchComposition::QUERY, lex.tx, search.query())
                                .id(SearchComposition::FIELD)
                                .label("Search")
                                .focusable()
                                .focus_ring(lex.mauve)
                                .min_width(12.0)
                                .claim(Component::Field)
                                .nowrap(),
                        )
                        .child(
                            Element::surface()
                                .width(SearchComposition::CARET.0)
                                .height(SearchComposition::CARET.1)
                                .fill(lex.mauve),
                        ),
                )
                .child(
                    Element::text(Voice::LEDGER_META, lex.ov0, count)
                        .claim(Component::Counter)
                        .nowrap(),
                ),
        )
        .child(hairline(glass, lex))
}

/// The scope band: one chip per scope, the picked one washed — over
/// the band's hairline.
fn chips(search: &Search, glass: Srgba, lex: &Lexicon) -> Element {
    let mut band = Element::row()
        .pad_x(SearchComposition::BAND_PAD_X)
        .pad_y(SearchComposition::BAND_PAD_Y)
        .gap(SearchComposition::CHIPS_GAP)
        .center();
    for (index, scope) in SearchScope::ALL.into_iter().enumerate() {
        band = band.child(ScopeChip::el(
            scope.label(),
            scope == search.scope(),
            glass,
            ElementId(SearchComposition::SCOPES + index as u64),
            lex,
        ));
    }
    Element::surface().child(band).child(hairline(glass, lex))
}

/// The hits under their kind caps, in the order the model grouped
/// them: a cap opens each kind, the chosen row is washed, and the air
/// under each row is the volume's.
fn rows(search: &Search, hits: &[SearchHit], glass: Srgba, lex: &Lexicon) -> Element {
    let mut block = Element::surface()
        .pad_x(SearchComposition::RESULTS_PAD_X)
        .pad_y(SearchComposition::RESULTS_PAD_TOP)
        .pad_bottom(SearchComposition::RESULTS_PAD_BOTTOM);
    let mut kind: Option<SearchKind> = None;
    for (index, hit) in hits.iter().enumerate() {
        if kind != Some(hit.kind) {
            kind = Some(hit.kind);
            block = block
                .child(cap(hit.kind, lex))
                .child(air(SearchComposition::CAP_BELOW));
        }
        let chosen = index == search.chosen();
        let last = index + 1 == hits.len();
        let (voice, ink) = line_voice(hit.kind, lex);
        let line = match &hit.when {
            Some(when) => format!("{} \u{b7} {when}", hit.line),
            None => hit.line.clone(),
        };
        block = block.child(SearchRow::el(
            hit.title.as_deref(),
            MatchLine::el(&line, &hit.marks, voice, ink, lex),
            chosen,
            if last {
                SearchComposition::LAST_ROW_PAD_BOTTOM
            } else {
                SearchComposition::ROW_PAD_BOTTOM
            },
            glass,
            ElementId(SearchComposition::ROWS + index as u64),
            lex,
        ));
        if !last {
            block = block.child(air(if chosen {
                SearchComposition::ROW_BELOW_CHOSEN
            } else {
                SearchComposition::ROW_BELOW
            }));
        }
    }
    block
}

/// A kind's cap: its glyph and its word, in the volume's cap voice.
fn cap(kind: SearchKind, lex: &Lexicon) -> Element {
    let icon = match kind {
        SearchKind::Report => Icon::Report,
        SearchKind::Trace => Icon::Trace,
        SearchKind::Receipt => Icon::Commit,
        SearchKind::Session => Icon::Session,
    };
    Element::row()
        .gap(SearchComposition::CAP_GAP)
        .center()
        .child(Element::mark(
            Mark::Icon(icon),
            SearchComposition::CAP_ICON_PX,
            Lexicon::ink_bytes(lex.ov0),
        ))
        .child(
            Element::text(Voice::CAP, lex.ov0, kind.word())
                .claim(Component::Cap)
                .nowrap(),
        )
}

/// The voice a kind's line is set in: a chapter's snippet in the
/// faint mono 10, a step's, a receipt's or a session's identity in the
/// subtle mono 10.5.
fn line_voice(kind: SearchKind, lex: &Lexicon) -> (Voice, Srgba) {
    match kind {
        SearchKind::Report => (Voice::LEDGER_META, lex.ov0),
        SearchKind::Trace | SearchKind::Receipt | SearchKind::Session => (Voice::CHIP, lex.sb0),
    }
}

/// A band's closing hairline, on the glass.
fn hairline(glass: Srgba, lex: &Lexicon) -> Element {
    Element::surface()
        .height(1.0)
        .fill(Lexicon::composite(glass, lex.hairc))
}

/// A rigid run of vertical air.
fn air(px: f32) -> Element {
    Element::surface().height(px)
}

#[cfg(test)]
#[path = "comp_search.tests.rs"]
mod tests;
