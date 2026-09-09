use magistr_cockpit::cockpit::{Chapter, Model, Shelf, StillKind};

use crate::action::Action;
use crate::action_row::ActionRow;
use crate::chapter_title::ChapterTitle;
use crate::comp_shelf::ShelfComposition;
use crate::component::Component;
use crate::deliverable_row::DeliverableRow;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::icon::Icon;
use crate::lexicon::Lexicon;
use crate::mark::Mark;
use crate::medium::Medium;
use crate::receipt_row::ReceiptRow;
use crate::still_tile::StillTile;
use crate::voice::Voice;

/// The Report, composed from the model: a shipped star opened. The chapter on the left
/// — the cap, the title in the display face, the prose in the chapter's serif, the
/// Unlocked list — and the deliverables rail on the right: the stills as tiles, the
/// recording and the filmstrips as rows, the receipts as receipt rows with their verbs.
/// The header borrows [`ReportComposition::chips`] for its right-hand slot: the two
/// verbs, `Share page` in the accent and `Export bundle`.
///
/// Everything here is the model's: the [`Chapter`] is derived from the roadmap on every
/// read and the [`Shelf`] is what the entry point's job gathered. A shelf not yet
/// landed is composed as the rail with its rows saying so — the page never invents a
/// receipt.
///
/// Motion: none. An archive page holds still, so nothing here reads the tick.
pub struct ReportComposition;

impl ReportComposition {
    /// `Share page` — the accent verb.
    pub const SHARE: ElementId = ElementId(360);
    /// The commit receipt's `View`.
    pub const COMMIT: ElementId = ElementId(361);
    /// The gates receipt's `Log`.
    pub const GATES: ElementId = ElementId(362);
    /// The cost receipt's `Detail`.
    pub const COST: ElementId = ElementId(363);
    /// The trace receipt's `Open` — the provenance chip.
    pub const TRACE: ElementId = ElementId(364);

    /// The view's header band: the record band's padding (`15px 20px
    /// 12px 92px`) around the two 32-px buttons, its hairline on the
    /// capture's row 59.
    pub(crate) const HEADER_HEIGHT: f32 = 60.0;
    /// The header verbs' pitch (`gap:14px`).
    const CHIP_GAP: f32 = 14.0;

    /// The chapter's margins (`padding:26px 34px`) and its measure
    /// (`max-width:520px`).
    const PAD_X: f32 = 34.0;
    const PAD_Y: f32 = 26.0;
    const MEASURE: f32 = 520.0;
    /// The air under the cap (`margin-bottom:8px`), the title (12) and
    /// the prose (12).
    const CAP_BELOW: f32 = 8.0;
    const TITLE_BELOW: f32 = 12.0;
    const PROSE_BELOW: f32 = 12.0;
    /// The cap's glyph (`.cap7 svg.ic { width:11px }`) and its gap.
    const CAP_ICON_PX: u32 = 11;
    const CAP_GAP: f32 = 4.0;

    /// The rail (`width:330px; padding:22px`, `hair-l`).
    pub(crate) const RAIL_WIDTH: f32 = 330.0;
    const RAIL_PAD: f32 = 22.0;
    /// The air under the Deliverables cap (10), the tiles (12), the
    /// recording row (8), the filmstrips row (14) and the Receipts cap
    /// (6); the tiles' gap (8).
    const DELIVERABLES_BELOW: f32 = 10.0;
    const TILES_BELOW: f32 = 12.0;
    const TILE_GAP: f32 = 8.0;
    const RECORDING_BELOW: f32 = 8.0;
    const FILMSTRIPS_BELOW: f32 = 14.0;
    const RECEIPTS_BELOW: f32 = 6.0;
    /// Stills the rail tiles — the volume's two; the rest are in the
    /// bundle and counted on the Sky's gallery.
    const TILES: usize = 2;

    /// The chapter's prose voice — the volume's serif 14 on 1.7, the chapter's own body
    /// face beside the title's [`Voice::DISPLAY`]. The serif's one home is the chapter;
    /// this is that home's second seat, and nothing outside this composition sets it.
    pub const PROSE: Voice = Voice {
        family: "serif",
        px: 14.0,
        weight: 400,
        tracking_em: 0.0,
        line_height: 1.7,
        upper: false,
    };
    /// The Unlocked line's voice — the volume's `.mono` at 11.
    const UNLOCKED: Voice = Voice {
        family: "mono",
        px: 11.0,
        weight: 400,
        tracking_em: 0.0,
        line_height: 0.0,
        upper: false,
    };

    /// The header's right-hand slot: `Share page` in the accent and
    /// `Export bundle` — the Report's two verbs.
    #[must_use]
    pub fn chips(medium: Medium) -> Element {
        ActionRow::new(
            None,
            vec![
                Action::new("Share page", Self::SHARE).accent(),
                Action::new("Export bundle", ShelfComposition::EXPORT),
            ],
        )
        .gap(Self::CHIP_GAP)
        .el(medium)
    }

    /// The view's body for `model`, in `medium`'s lexicon: the chapter
    /// column and the rail.
    #[must_use]
    pub fn el(model: &Model, medium: Medium) -> Element {
        let lex = Lexicon::of(medium);
        let chapter = model.chapter();
        Element::row()
            .grow(1.0)
            .min_height(0.0)
            .child(column(chapter.as_ref(), &lex))
            .child(rail(model.shelf(), &lex))
    }
}

/// The chapter column: the cap, the title, the prose, the Unlocked
/// list — at the measure.
fn column(chapter: Option<&Chapter>, lex: &Lexicon) -> Element {
    let (cap_text, title, prose, unlocked) = match chapter {
        Some(chapter) => (
            chapter.cap(),
            chapter.title.clone(),
            chapter.prose_text().to_owned(),
            chapter.unlocked_text(),
        ),
        // The camera can stand here with no shipped chunk under it (Tab
        // walks every place): the page says so in its own components.
        None => (
            "Report".to_owned(),
            "No report is open".to_owned(),
            "Open a shipped chunk on the Roadmap to read its chapter.".to_owned(),
            Chapter::NOTHING_UNLOCKED.to_owned(),
        ),
    };
    let column = Element::surface()
        .max_width(ReportComposition::MEASURE)
        .stretch()
        .align_start()
        .child(cap(Some(Icon::Report), &cap_text, lex))
        .child(air(ReportComposition::CAP_BELOW))
        .child(ChapterTitle::el(&title, lex).stretch())
        .child(air(ReportComposition::TITLE_BELOW))
        .child(
            Element::text(ReportComposition::PROSE, lex.sb1, prose)
                .claim(Component::Prose)
                .stretch()
                .min_width(0.0),
        )
        .child(air(ReportComposition::PROSE_BELOW))
        .child(cap(Some(Icon::Roadmap), "Unlocked", lex))
        .child(air(ReportComposition::CAP_BELOW))
        .child(
            Element::text(ReportComposition::UNLOCKED, lex.sb0, unlocked)
                .claim(Component::Prose)
                .nowrap(),
        );
    Element::surface()
        .grow(1.0)
        .min_height(0.0)
        .min_width(0.0)
        .pad_x(ReportComposition::PAD_X)
        .pad_y(ReportComposition::PAD_Y)
        .align_start()
        .child(column)
}

/// The deliverables rail: the cap, the still tiles, the recording and
/// the filmstrips rows, the receipts — behind its left hairline.
fn rail(shelf: Option<&Shelf>, lex: &Lexicon) -> Element {
    let inner = ReportComposition::RAIL_WIDTH - 1.0 - 2.0 * ReportComposition::RAIL_PAD;
    let mut column = Element::surface()
        .width(ReportComposition::RAIL_WIDTH - 1.0)
        .pad(ReportComposition::RAIL_PAD)
        .align_start()
        .child(cap(Some(Icon::Stack), "Deliverables", lex))
        .child(air(ReportComposition::DELIVERABLES_BELOW));
    let stills: Vec<_> = shelf
        .map(|shelf| {
            shelf
                .stills
                .iter()
                .filter(|still| still.kind == StillKind::Still)
                .take(ReportComposition::TILES)
                .collect()
        })
        .unwrap_or_default();
    if stills.is_empty() {
        column = column.child(
            Element::text(Voice::CAP, lex.sb0, "no stills kept")
                .claim(Component::Cap)
                .nowrap(),
        );
    } else {
        #[allow(clippy::cast_precision_loss)]
        let width = (inner - ReportComposition::TILE_GAP * (stills.len() as f32 - 1.0))
            / stills.len() as f32;
        let mut tiles = Element::row().gap(ReportComposition::TILE_GAP).width(inner);
        for still in stills {
            tiles = tiles.child(StillTile::el(still, width.floor(), lex));
        }
        column = column.child(tiles);
    }
    let recording = shelf.map_or_else(|| "not recorded".to_owned(), Shelf::recording_text);
    let films = shelf.map_or(0, |shelf| shelf.count(StillKind::Filmstrip));
    column = column
        .child(air(ReportComposition::TILES_BELOW))
        .child(
            DeliverableRow::el(
                Icon::Recording,
                lex.mauve,
                true,
                "Session recording",
                &recording,
                lex,
            )
            .width(inner),
        )
        .child(air(ReportComposition::RECORDING_BELOW))
        .child(
            DeliverableRow::el(
                Icon::Filmstrip,
                lex.teal,
                false,
                "Motion filmstrips",
                &format!("{films} {}", if films == 1 { "strip" } else { "strips" }),
                lex,
            )
            .width(inner),
        )
        .child(air(ReportComposition::FILMSTRIPS_BELOW))
        .child(cap(None, "Receipts", lex))
        .child(air(ReportComposition::RECEIPTS_BELOW))
        .child(receipts(shelf, inner, lex));
    Element::row()
        .width(ReportComposition::RAIL_WIDTH)
        .no_shrink()
        .child(
            Element::surface()
                .width(1.0)
                .stretch()
                .fill(lex.grounded(lex.hairc)),
        )
        .child(column)
}

/// The receipt rows: commit · gates · cost · trace, each with its verb,
/// and — once a bundle or a page was written — where it went. A shelf
/// not yet landed shows the four rows saying so, with their verbs, so
/// the page has the same shape before and after the job answers.
fn receipts(shelf: Option<&Shelf>, inner: f32, lex: &Lexicon) -> Element {
    let bare = Shelf::default();
    let shelf = shelf.unwrap_or(&bare);
    let hash = shelf
        .commit
        .as_ref()
        .map_or_else(|| "none found".to_owned(), |(hash, _)| hash.clone());
    let lit: Vec<bool> = shelf.gates.iter().map(|(_, passed)| *passed).collect();
    let more = shelf.bundle.is_some() || shelf.page.is_some();
    let mut rows = Element::surface()
        .width(inner)
        .child(ReceiptRow::linked(
            (Icon::Commit, "commit"),
            &hash,
            "View",
            ReportComposition::COMMIT,
            true,
            lex.base,
            lex,
        ))
        .child(ReceiptRow::lit_linked(
            (Icon::Gate, "gates"),
            &lit,
            &shelf.gates_text(),
            "Log",
            ReportComposition::GATES,
            true,
            lex.base,
            lex,
        ))
        .child(ReceiptRow::linked(
            (Icon::Cost, "cost"),
            &shelf.spend_text(),
            "Detail",
            ReportComposition::COST,
            true,
            lex.base,
            lex,
        ))
        .child(ReceiptRow::linked(
            (Icon::Trace, "trace"),
            &shelf.trace_refs_text(),
            "Open",
            ReportComposition::TRACE,
            more,
            lex.base,
            lex,
        ));
    if let Some(bundle) = &shelf.bundle {
        rows = rows.child(ReceiptRow::value(
            (Icon::Folder, "bundle"),
            &tail(bundle),
            None,
            shelf.page.is_some(),
            lex.base,
            lex,
        ));
    }
    if let Some(page) = &shelf.page {
        rows = rows.child(ReceiptRow::value(
            (Icon::Report, "page"),
            &tail(page),
            None,
            false,
            lex.base,
            lex,
        ));
    }
    rows
}

/// A section cap in the volume's own voice, with its glyph when it
/// wears one.
fn cap(icon: Option<Icon>, text: &str, lex: &Lexicon) -> Element {
    let mut row = Element::row().gap(ReportComposition::CAP_GAP).center();
    if let Some(icon) = icon {
        row = row.child(Element::mark(
            Mark::Icon(icon),
            ReportComposition::CAP_ICON_PX,
            Lexicon::ink_bytes(lex.ov0),
        ));
    }
    row.child(
        Element::text(Voice::CAP, lex.ov0, text)
            .claim(Component::Cap)
            .nowrap(),
    )
}

/// The last three components of `path` — all a value slot has room
/// for.
fn tail(path: &std::path::Path) -> String {
    let parts: Vec<String> = path
        .components()
        .rev()
        .take(3)
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    parts.into_iter().rev().collect::<Vec<_>>().join("/")
}

/// A rigid run of vertical air.
fn air(px: f32) -> Element {
    Element::surface().height(px)
}

#[cfg(test)]
#[path = "comp_report.tests.rs"]
mod tests;
