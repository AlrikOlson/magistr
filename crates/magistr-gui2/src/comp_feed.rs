use crate::rich_text::RichText;
use magistr_application::sessions::{ToolRecord, ToolStatus, ToolVerb};
use magistr_cockpit::cockpit::{Model, Term};
use magistr_cockpit::turn::{Line, LineKind, Receipt, Row, RowKind, Voice as TurnVoice, diff_kind};
use qs_gpu::color::Srgba;
use serde_json::Value;

use crate::breath::Breath;
use crate::card::Card;
use crate::component::Component;
use crate::diff_row::DiffRow;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::icon::Icon;
use crate::lexicon::Lexicon;
use crate::mark::Mark;
use crate::medium::Medium;
use crate::tool_detail::ToolDetail;
use crate::voice::Voice;

/// The turn's leaves, composed from the model: the windowed rows the cockpit already
/// holds, arranged the way view 05 draws them — a paragraph as one run the shaper
/// breaks at the measure, a tool or edit line's rows floated as one card wearing the
/// volume's anatomy (head · body · receipt strip), a diff row in the diff vocabulary,
/// an image's footprint reserved at its placement's size. The window and the grouping
/// are the model's ([`Model::visible_rows`],
/// [`Turn::card_at_row`](magistr_cockpit::turn::Turn::card_at_row),
/// [`Turn::prose_at_row`](magistr_cockpit::turn::Turn::prose_at_row)); only a
/// paragraph's line break is this crate's, because the model counts mono columns and
/// the window draws the volume's sans.
pub struct FeedComposition;

impl FeedComposition {
    /// The band's tint over the head as the kill is held: the failure hue at a third,
    /// so the head's words stay legible on it up to the kill.
    fn kill_tint(lex: &Lexicon) -> Srgba {
        Srgba::new(lex.red.r, lex.red.g, lex.red.b, Self::KILL_ALPHA)
    }
    /// The air over a floated card — the paragraph's own margin under
    /// it (`p { margin:0 0 10px }`) — and the card's own margin under
    /// itself (`.float7 { margin-bottom:8px }`).
    const AIR_ABOVE: f32 = 14.0;
    const AIR_BELOW: f32 = 12.0;
    /// The feed's card corner — the session view's own
    /// `border-radius:11px`, one step under `.float7`'s 13.
    const CARD_RADIUS: f32 = 11.0;
    /// The inset a card's text sits at — the head's `padding:6px 12px`
    /// and the diff line's `padding:0 12px` agree on it.
    const CARD_PAD_X: f32 = 12.0;
    /// The head row's height on the capture: the 10.5 mono on its line
    /// box between the 6px pads, with the 13px verb icon centred in it.
    const HEAD_HEIGHT: f32 = 29.0;
    /// The head's `gap:8px` between verb, subject and measure.
    const HEAD_GAP: f32 = 8.0;
    /// The verb's `gap:4px` between its icon and its word.
    const VERB_GAP: f32 = 4.0;
    /// The verb icon at the set's own `.ic` size.
    const VERB_ICON_PX: u32 = 13;
    /// How many changed lines a collapsed edit card previews under its
    /// head before the receipt strip.
    const PREVIEW_LINES: usize = 2;
    /// The shortest a cut title gets, in cells, whatever the measure.
    const TITLE_FLOOR: usize = 8;
    /// The id space an edit card's button row indexes into, by the
    /// row's absolute index: `BUTTONS + 2·at` is [Open],
    /// `BUTTONS + 2·at + 1` is [Undo].
    pub const BUTTONS: u64 = 1600;
    /// The id space a card's head wears, by the head's absolute row —
    /// the hit target a click or ↵ opens the card through, routed to
    /// the cockpit's own stage-row toggle so the terminal's expansion
    /// and the window's are one state. Above the button space, under
    /// the images.
    pub const HEADS: u64 = 1 << 39;
    /// The kill band's alpha over the card's ground.
    const KILL_ALPHA: f32 = 0.34;
    /// The id space an image reservation wears, by the placement's
    /// head row — how the frame finds the footprint to place the
    /// picture's quad in. Far above every row-indexed space, so the
    /// two can never meet.
    pub const IMAGES: u64 = 1 << 40;

    /// How many MONO cells of text a card in this feed holds, given the
    /// feed's own box and the surface's cell advance — both in the
    /// volume's logical pixels.
    ///
    /// The model cuts a tool head and breaks code in COLUMNS, and the
    /// window measures its turn column in PIXELS, so somebody converts.
    /// It is this module, because the number that comes off the box is
    /// the card's own inset — this module's token. Prose is not
    /// counted here: it is drawn in the volume's narrower sans and
    /// keeps the wider count, which is the whole reason the two widths
    /// are separate (`Turn::set_mono_width`).
    #[must_use]
    pub fn mono_cols(width: f32, cell: f32) -> u16 {
        if !(width.is_finite() && cell > 0.0) {
            return 1;
        }
        let text = width - 2.0 * Self::CARD_PAD_X;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let cols = (text / cell).floor().max(1.0) as u32;
        u16::try_from(cols).unwrap_or(u16::MAX)
    }

    /// The placement head row behind an image reservation's id, if it
    /// is one.
    #[must_use]
    pub fn image_row(id: ElementId) -> Option<usize> {
        usize::try_from(id.0.checked_sub(Self::IMAGES)?).ok()
    }

    /// The feed for `model`, in `medium`'s lexicon. `cell` is the
    /// surface's cell advance and row height in pixels — an image's
    /// footprint is measured in the model's cells, and only the
    /// renderer knows their pixel size.
    #[must_use]
    pub fn el(model: &Model, medium: Medium, cell: (f32, f32)) -> Element {
        let lex = Lexicon::of(medium);
        let top = model.top_row();
        let rows = model.visible_rows();
        // The arrival transient is the cockpit's, not this crate's: the
        // cell painter fades a new row in over the same three steps
        // from the same stamp, so the two surfaces cannot disagree
        // about when a row is new. Under reduced motion the gate is
        // shut and every row is painted settled on its first frame.
        let arriving = model
            .theme()
            .motion()
            .is_full()
            .then(|| (model.motion(), model.now()));
        // The working breath: a running tool or edit card wears the halo; the last row,
        // while it is a thought being thought and no card is running, lifts its ink.
        // `None` on a terminal, under reduced motion, when nothing is working or while
        // a decision waits on the human.
        let breath = Breath::of(model);
        let running_card = rows.iter().any(|row| {
            matches!(
                row.kind,
                RowKind::Tool(magistr_application::sessions::ToolStatus::InProgress)
            )
        });
        // Stretch, not start: a row sized to its own content can be
        // wider than the column it sits in, which is how a long command
        // used to carry its card off the stage's right edge. The
        // capture's own anatomy agrees — an edit card spans the turn's
        // measure rather than hugging its head text.
        let mut column = Element::surface();
        let mut index = 0;
        while index < rows.len() {
            let at = top.saturating_add(index);
            let fade = arriving.and_then(|(m, now)| m.row_arrival(at, now));
            if let Some((line, span)) = model.turn().card_at_row(at) {
                let visible = span.min(rows.len() - index);
                let float = card(
                    model,
                    line,
                    &rows[index..index + visible],
                    at,
                    &lex,
                    medium,
                    fade,
                );
                let float = match (breath, running(line)) {
                    (Some(breath), true) => breath.halo(float, Self::CARD_RADIUS, medium, &lex),
                    _ => float,
                };
                column = column
                    .child(air(Self::AIR_ABOVE))
                    .child(float.no_shrink())
                    .child(air(Self::AIR_BELOW));
                index += visible;
                continue;
            }
            // A paragraph: the model broke it into rows by counting
            // mono columns, and the window draws it in the volume's
            // sans at the column's 600px measure — so its visible rows
            // are joined back into one run and the shaper that is
            // actually drawing it breaks it where the capture breaks.
            // The fade rides the paragraph's first visible row: a
            // paragraph fades in when it appears, and grows at full ink.
            if let Some((_, span)) = model.turn().prose_at_row(at) {
                let visible = span.min(rows.len() - index);
                let slice = &rows[index..index + visible];
                if slice
                    .iter()
                    .all(|row| matches!(row.kind, RowKind::Prose(_)))
                {
                    let thinking = breath.filter(|_| {
                        index + visible == rows.len()
                            && !running_card
                            && matches!(slice[0].kind, RowKind::Prose(TurnVoice::Thought))
                    });
                    column = column.child(paragraph(slice, &lex, fade, thinking).no_shrink());
                    index += visible;
                    continue;
                }
            }
            let row = &rows[index];
            let thinking = breath.filter(|_| {
                index + 1 == rows.len()
                    && !running_card
                    && matches!(row.kind, RowKind::Prose(TurnVoice::Thought))
            });
            column = column.child(
                match row.kind {
                    RowKind::Image => image_slot(model, at, cell),
                    RowKind::EditButtons => buttons(at, medium),
                    _ => prose(row, &lex, fade, thinking),
                }
                .no_shrink(),
            );
            index += 1;
        }
        column
    }
}

/// A rigid run of vertical air — rigid both ways: it neither grows nor
/// gives its pixels up when the feed's window overfills its box.
fn air(px: f32) -> Element {
    Element::surface().height(px).no_shrink()
}

/// A paragraph's visible rows as one run: the rows are exact substrings
/// the model cut at spaces, so a single space between them is the text
/// the agent sent.
fn paragraph(rows: &[Row], lex: &Lexicon, fade: Option<f32>, thinking: Option<Breath>) -> Element {
    let mut text = String::new();
    let mut spans = Vec::new();
    for row in rows {
        if !text.is_empty() {
            text.push(' ');
        }
        let offset = text.len();
        spans.extend(row.spans.iter().map(|span| span.shifted(offset)));
        text.push_str(row.text.trim_end());
    }
    let row = Row::styled(rows[0].kind, text, spans);
    let color = ink(row.kind, lex);
    let color = thinking.map_or(color, |breath| breath.ink(color, lex.tx));
    crate::rich_text::RichText::row(&row, arriving(color, lex, fade), lex)
}

/// One run of `text` in the voice its kind names and the ink its kind
/// earns — [`RowKind::text_role`] is the one classifier, so the feed
/// and the cell painter cannot disagree about what is mono.
fn prose(row: &Row, lex: &Lexicon, fade: Option<f32>, thinking: Option<Breath>) -> Element {
    // No `nowrap` here, deliberately: a run that overruns the measure
    // is re-broken by the shaper that is actually drawing it — which
    // for a joined paragraph is the whole point.
    let ink = ink(row.kind, lex);
    // A thought being thought breathes between its faint pen and the
    // text pen: light, not movement, and nothing at the foot.
    let ink = match thinking {
        Some(breath) => breath.ink(ink, lex.tx),
        None => ink,
    };
    RichText::row(row, arriving(ink, lex, fade), lex).claim(Component::Prose)
}

/// Whether `line` is a tool or edit still running — the working thing
/// the breathe halo is for.
fn running(line: &Line) -> bool {
    matches!(
        line.kind(),
        LineKind::Tool {
            status: ToolStatus::InProgress,
            ..
        } | LineKind::Edit {
            status: ToolStatus::InProgress,
            ..
        }
    )
}

/// `ink` as it paints while its row arrives: mixed back toward the dim
/// pen by how much of the fade is left, exactly itself once the row has
/// settled — so nothing that is not arriving pays for this at all, and
/// a settled feed is the feed it always was, byte for byte. The cell
/// painter's [`Pen::fade`](magistr_cockpit::theme::Pen::fade) walks the
/// same path from the same stamp; here the walk is in Oklab, the space
/// the volume mixes its own faces in.
fn arriving(ink: Srgba, lex: &Lexicon, fade: Option<f32>) -> Srgba {
    match fade {
        Some(t) => crate::oklab::mix(ink, lex.sb0, t.clamp(0.0, 1.0)),
        None => ink,
    }
}

/// The ink a row's kind earns — the semantic pens of the cell feed,
/// read off the lexicon.
fn ink(kind: RowKind, lex: &Lexicon) -> Srgba {
    match kind {
        RowKind::Prose(TurnVoice::Thought) | RowKind::Fence | RowKind::Event => lex.ov0,
        RowKind::Prose(TurnVoice::User) => lex.sb1,
        RowKind::Check(true) => lex.green,
        RowKind::Check(false) | RowKind::Error | RowKind::Conflict => lex.red,
        RowKind::DiffAdd => lex.green,
        RowKind::DiffRemove => lex.red,
        RowKind::Note => lex.sb0,
        _ => lex.tx,
    }
}

/// A tool, check or edit line's rows as one floated card in the volume's one anatomy:
/// the head — verb · subject · measure — over a hairline, the body in the data voice,
/// and the receipt line. Collapsed, an edit previews its first changed lines and a tool
/// or check is its head alone; expanded, an edit's rows ride inside the float as the
/// model laid them — the button row, then the diff in the diff vocabulary — and a tool
/// or check opens into its verb's [`ToolDetail`], composed from the record the line
/// keeps rather than from the grid's wrapped rows. The expansion itself is the line's
/// (`LineKind::expanded`), so what is open here is what is open on a terminal.
fn card(
    model: &Model,
    line: &Line,
    rows: &[Row],
    at: usize,
    lex: &Lexicon,
    medium: Medium,
    fade: Option<f32>,
) -> Element {
    // The card's ground rises out of the CANVAS with its ink, so a tool
    // call materializes rather than snapping into place. Toward the
    // canvas, never toward the dim pen: that is where a surface comes
    // from, and mixing a ground toward a text colour paints a bright
    // bar where a card should be.
    let ground = match fade {
        Some(t) => crate::oklab::mix(lex.mantle, lex.base, t.clamp(0.0, 1.0)),
        None => lex.mantle,
    };
    let receipt = call_id(line).and_then(|id| model.turn().receipt(id));
    let mut float = Card::el(0.0, 0.0, lex)
        .radius(FeedComposition::CARD_RADIUS)
        .fill(ground)
        .child(head(model, line, receipt, at, ground, lex, fade))
        .child(
            Element::surface()
                .height(1.0)
                .fill(Lexicon::composite(ground, lex.hairc)),
        );
    match line.kind() {
        LineKind::Edit { undone, .. } => {
            if rows.len() > 1 {
                for (offset, row) in rows.iter().enumerate().skip(1) {
                    float = float.child(match row.kind {
                        RowKind::DiffAdd => DiffRow::add(&row.text, ground, lex),
                        RowKind::DiffRemove => DiffRow::del(&row.text, ground, lex),
                        RowKind::EditButtons => buttons(at + offset, medium),
                        _ => DiffRow::ctx(&row.text, lex),
                    });
                }
            } else {
                for spelled in preview(line.text()) {
                    float = float.child(match diff_kind(&spelled) {
                        RowKind::DiffAdd => DiffRow::add(&spelled, ground, lex),
                        RowKind::DiffRemove => DiffRow::del(&spelled, ground, lex),
                        _ => DiffRow::ctx(&spelled, lex),
                    });
                }
            }
            float.child(strip(receipt, *undone, lex))
        }
        LineKind::Tool { expanded: true, .. } | LineKind::Check { expanded: true, .. } => float
            .child(detail(model, line, ground, lex))
            .child(receipt_line(
                line,
                receipt,
                killed_by_reader(model, line),
                ground,
                lex,
            )),
        _ => float,
    }
}

/// The call behind a card, for its receipt.
fn call_id(line: &Line) -> Option<&str> {
    match line.kind() {
        LineKind::Tool { id, .. } | LineKind::Edit { id, .. } => Some(id),
        _ => None,
    }
}

/// A collapsed edit's preview: the body's first changed lines, spelled
/// the volume's way — the marker, a space, the code.
fn preview(body: &str) -> impl Iterator<Item = String> + '_ {
    body.lines()
        .filter(|line| matches!(diff_kind(line), RowKind::DiffAdd | RowKind::DiffRemove))
        .take(FeedComposition::PREVIEW_LINES)
        .map(|line| format!("{} {}", &line[..1], line[1..].trim_start()))
}

/// What a head and a receipt say of a program the reader ended — the same words the
/// terminal's strip row uses.
const KILLED_BY_YOU: &str = "killed by you";

/// The receipt strip under an edit card: what the receipt names about its rollback —
/// `rollback available · inside cp-4's reach`, or only the rollback while nothing is
/// checkpointed, or `rolled back` once the journal rewound it. Set as a context
/// diffline, the way the volume draws it.
fn strip(receipt: Option<&Receipt>, undone: bool, lex: &Lexicon) -> Element {
    let text = if undone {
        "  rolled back".to_owned()
    } else {
        match receipt.and_then(|receipt| receipt.reach.as_deref()) {
            Some(reach) => format!("  rollback available \u{b7} inside {reach}\u{2019}s reach"),
            None => "  rollback available".to_owned(),
        }
    };
    DiffRow::ctx(&text, lex)
}

/// A card's head — verb · subject · measure: the verb's icon and word
/// in the verb's own hue (a state code: a failed call's verb is red, a
/// pending one's faint, a failed check's red), the subject in the
/// subdued ink cut to what the card holds, and at the far edge the
/// measure — an edit's `+47 · receipt #241` in green, a tool's `12 ms ·
/// receipt #238` in the faint ink, a check's `exit 0 · 1.2 s`. The
/// head is the card's hit target: it wears the row's id, takes focus,
/// and the arrow keys walk from it to the next card's.
fn head(
    model: &Model,
    line: &Line,
    receipt: Option<&Receipt>,
    at: usize,
    ground: Srgba,
    lex: &Lexicon,
    fade: Option<f32>,
) -> Element {
    let (icon, hue, word, subject) = match line.kind() {
        LineKind::Tool { verb, status, .. } => {
            let (icon, hue) = verb_mark(*verb, lex);
            let hue = match status {
                ToolStatus::Failed => lex.red,
                ToolStatus::Pending => lex.ov0,
                ToolStatus::InProgress | ToolStatus::Completed => hue,
            };
            (icon, hue, verb.label(), line.text())
        }
        LineKind::Edit { path, status, .. } => {
            let (icon, hue) = verb_mark(ToolVerb::Edit, lex);
            let hue = match status {
                ToolStatus::Failed => lex.red,
                ToolStatus::Pending => lex.ov0,
                ToolStatus::InProgress | ToolStatus::Completed => hue,
            };
            (icon, hue, ToolVerb::Edit.label(), path.as_str())
        }
        LineKind::Check { name, passed, .. } => (
            Icon::Gate,
            if *passed { lex.green } else { lex.red },
            "check",
            name.as_str(),
        ),
        _ => (Icon::Stack, lex.ov0, ToolVerb::Tool.label(), line.text()),
    };
    let verb_ink = arriving(hue, lex, fade);
    let measure = measure(line, receipt, killed_by_reader(model, line), lex);
    let budget = title_budget(
        model,
        word,
        measure.as_ref().map_or(0, |(text, _)| text.chars().count()),
    );
    let mut row = Element::row()
        .height(FeedComposition::HEAD_HEIGHT)
        .pad_x(FeedComposition::CARD_PAD_X)
        .gap(FeedComposition::HEAD_GAP)
        .center()
        .id(ElementId(FeedComposition::HEADS + at as u64))
        .walkable()
        .focus_ring(lex.mauve)
        .hover_fill(Lexicon::composite(ground, lex.hairc))
        .label(format!("{word} {subject}"));
    // The held kill fills the head red from the left as it is held, behind the head's
    // own words: a band pinned to the head's box, its filled share the hold's fraction.
    if let Some(fraction) = line
        .record()
        .terminal
        .as_deref()
        .and_then(|id| model.hold_fraction(id))
        .filter(|fraction| *fraction > 0.0)
    {
        row = row.child(
            Element::row()
                .pin_left(0.0)
                .pin_right(0.0)
                .pin_top(0.0)
                .pin_bottom(0.0)
                .child(
                    Element::surface()
                        .grow(fraction)
                        .fill(Lexicon::composite(ground, FeedComposition::kill_tint(lex))),
                )
                .child(Element::surface().grow(1.0 - fraction)),
        );
    }
    row = row
        .child(
            Element::row()
                .gap(FeedComposition::VERB_GAP)
                .center()
                .no_shrink()
                .child(Element::mark(
                    Mark::Icon(icon),
                    FeedComposition::VERB_ICON_PX,
                    Lexicon::ink_bytes(verb_ink),
                ))
                .child(
                    Element::text(Voice::CHIP, verb_ink, word)
                        .claim(Component::Value)
                        .nowrap(),
                ),
        )
        .child(
            // The subject yields before the measure does: a run's
            // flex minimum is its shaped width, and a measure that
            // grew (`killed by you`) would otherwise push past the
            // head's edge rather than clip the title.
            Element::text(
                Voice::CHIP,
                arriving(lex.sb0, lex, fade),
                cut(subject, budget),
            )
            .claim(Component::Value)
            .nowrap()
            .min_width(0.0),
        )
        .child(Element::row().grow(1.0));
    if let Some((text, ink)) = measure {
        row = row.child(
            Element::text(Voice::LEDGER_META, arriving(ink, lex, fade), text)
                .claim(Component::ReceiptRow)
                .nowrap()
                .no_shrink(),
        );
    }
    row
}

/// The verb's icon and hue.
fn verb_mark(verb: ToolVerb, lex: &Lexicon) -> (Icon, Srgba) {
    match verb {
        ToolVerb::Read => (Icon::Read, lex.blue),
        ToolVerb::Edit | ToolVerb::Delete | ToolVerb::Move => (Icon::Edit, lex.teal),
        ToolVerb::Search => (Icon::Search, lex.yellow),
        ToolVerb::Run => (Icon::Run, lex.green),
        ToolVerb::Think => (Icon::Trace, lex.mauve),
        ToolVerb::Fetch => (Icon::Read, lex.blue),
        ToolVerb::Mode => (Icon::Settings, lex.sb0),
        ToolVerb::Tool => (Icon::Stack, lex.peach),
    }
}

/// Whether the reader ended the card's program with the held kill — read off the screen
/// the moment it happens, so the head says so before the call's receipt is minted.
fn killed_by_reader(model: &Model, line: &Line) -> bool {
    line.record()
        .terminal
        .as_deref()
        .and_then(|id| model.term(id))
        .is_some_and(Term::is_killed)
}

/// The head's measure: an edit's added and removed counts — off its
/// receipt once it has one, off its body while it runs — with the
/// receipt's ordinal; a tool's duration and ordinal; a check's exit
/// code and duration; `killed by you` ahead of either once the reader
/// ended the program. `None` while there is nothing to measure yet.
fn measure(
    line: &Line,
    receipt: Option<&Receipt>,
    killed: bool,
    lex: &Lexicon,
) -> Option<(String, Srgba)> {
    match line.kind() {
        LineKind::Edit { .. } => {
            let (added, removed) = receipt.map_or_else(
                || counts(line.text()),
                |receipt| (receipt.added, receipt.removed),
            );
            if added == 0 && removed == 0 && receipt.is_none() {
                return None;
            }
            let mut text = format!("+{added}");
            if removed > 0 {
                text.push_str(&format!(" \u{2212}{removed}"));
            }
            if let Some(receipt) = receipt {
                text.push_str(&format!(" \u{b7} receipt #{}", receipt.ordinal));
            }
            Some((text, lex.green))
        }
        LineKind::Tool { duration_ms, .. } => {
            let mut parts = Vec::new();
            if killed {
                parts.push(KILLED_BY_YOU.to_owned());
            }
            if let Some(ms) = duration_ms {
                parts.push(duration(*ms));
            }
            if let Some(receipt) = receipt {
                parts.push(format!("receipt #{}", receipt.ordinal));
            }
            let ink = if killed { lex.red } else { lex.ov0 };
            (!parts.is_empty()).then(|| (parts.join(" \u{b7} "), ink))
        }
        LineKind::Check {
            exit_code,
            duration_ms,
            ..
        } => {
            let mut parts = Vec::new();
            if killed {
                parts.push(KILLED_BY_YOU.to_owned());
            }
            if let Some(code) = exit_code {
                parts.push(format!("exit {code}"));
            }
            if let Some(ms) = duration_ms {
                parts.push(duration(*ms));
            }
            let ink = if killed { lex.red } else { lex.ov0 };
            (!parts.is_empty()).then(|| (parts.join(" \u{b7} "), ink))
        }
        _ => None,
    }
}

/// The lines a diff body adds and removes.
fn counts(body: &str) -> (u32, u32) {
    body.lines()
        .fold((0, 0), |(added, removed), line| match diff_kind(line) {
            RowKind::DiffAdd => (added + 1, removed),
            RowKind::DiffRemove => (added, removed + 1),
            _ => (added, removed),
        })
}

/// A duration in the volume's units : `12 ms` under a second, `1.2 s`
/// from there on.
fn duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms} ms")
    } else {
        format!("{}.{} s", ms / 1000, (ms % 1000) / 100)
    }
}

/// An expanded tool or check card's body: its verb's [`ToolDetail`],
/// composed from what the line keeps — the record's raw input and
/// output for a tool (the Claude Code CLI's own field names, the
/// title standing in where the wire named nothing), the details text
/// for a check — so the window shows the call the way the wire
/// described it while the grid keeps its wrapped title.
fn detail(model: &Model, line: &Line, ground: Srgba, lex: &Lexicon) -> Element {
    let record = line.record();
    let input_value = record.input.as_ref().map(payload_value);
    let input = input_value.as_ref();
    let output = output_text(record);
    // A call that embeds a terminal has its output on the screen the terminal streamed,
    // not in the record: the body is that screen's cells, drawn at this frame's tick so
    // the cursor blinks — and held still under reduced motion.
    let term = record
        .terminal
        .as_deref()
        .and_then(|id| model.term(id))
        .map(|term| (term, model.ticks(), !model.theme().motion().is_full()));
    let title = line.text();
    let path = record
        .locations
        .first()
        .map(|(path, _)| path.clone())
        .or_else(|| field(input, &["file_path", "path"]).map(str::to_owned))
        .unwrap_or_else(|| title.to_owned());
    let thought = field(input, &["thought"]).map_or_else(
        || {
            if record.text.is_empty() {
                if output.is_empty() {
                    title.to_owned()
                } else {
                    output.clone()
                }
            } else {
                record.text.join("\n")
            }
        },
        str::to_owned,
    );
    let sent = input.map(value_text).unwrap_or_default();
    let detail = match line.kind() {
        LineKind::Check { .. } => ToolDetail::Check {
            command: input.and_then(Value::as_str),
            rows: title,
            term,
        },
        LineKind::Tool {
            verb: ToolVerb::Run,
            ..
        } => ToolDetail::Run {
            command: field(input, &["command", "cmd"]).unwrap_or(title),
            output: &output,
            term,
        },
        LineKind::Tool {
            verb: ToolVerb::Read,
            ..
        } => ToolDetail::Read {
            path: &path,
            range: record
                .locations
                .first()
                .and_then(|(_, line)| *line)
                .or_else(|| number(input, "offset"))
                .map(|from| (from, number(input, "limit"))),
            excerpt: &output,
        },
        LineKind::Tool {
            verb: ToolVerb::Search,
            ..
        } => ToolDetail::Search {
            query: field(input, &["pattern", "query", "regex", "glob"]).unwrap_or(title),
            hits: &output,
        },
        LineKind::Tool {
            verb: ToolVerb::Fetch,
            ..
        } => ToolDetail::Fetch {
            url: field(input, &["url"]).unwrap_or(title),
            body: &output,
        },
        LineKind::Tool {
            verb: ToolVerb::Think,
            ..
        } => ToolDetail::Think { thought: &thought },
        _ => {
            let (server, tool) = mcp_name(title);
            ToolDetail::Mcp {
                server,
                tool,
                input: &sent,
                output: &output,
            }
        }
    };
    detail.el(ground, lex)
}

/// The receipt line under an open card: how the call ended (its exit
/// code when the output carried one, `failed`), how long it took, the
/// terminal it embeds, its receipt's ordinal — and the record's cut,
/// when it was cut.
fn receipt_line(
    line: &Line,
    receipt: Option<&Receipt>,
    killed: bool,
    ground: Srgba,
    lex: &Lexicon,
) -> Element {
    let record = line.record();
    let mut parts = Vec::new();
    // The reader's kill leads the receipt, whatever the exit code says.
    if killed || receipt.is_some_and(|receipt| receipt.killed) {
        parts.push(KILLED_BY_YOU.to_owned());
    }
    match line.kind() {
        LineKind::Tool {
            status,
            duration_ms,
            ..
        } => {
            if let Some(code) = exit_code(record) {
                parts.push(format!("exit {code}"));
            }
            if *status == ToolStatus::Failed {
                parts.push("failed".to_owned());
            }
            if let Some(ms) = duration_ms {
                parts.push(duration(*ms));
            }
            if let Some(id) = &record.terminal {
                parts.push(format!("terminal {id}"));
            }
        }
        LineKind::Check {
            passed,
            exit_code,
            duration_ms,
            ..
        } => {
            parts.push(if *passed { "passed" } else { "failed" }.to_owned());
            if let Some(code) = exit_code {
                parts.push(format!("exit {code}"));
            }
            if let Some(ms) = duration_ms {
                parts.push(duration(*ms));
            }
        }
        _ => {}
    }
    if let Some(receipt) = receipt {
        parts.push(format!("receipt #{}", receipt.ordinal));
    }
    ToolDetail::receipt(&parts, record.truncated_from, ground, lex)
}

/// What a call returned, as text: the raw output spelled out, or the
/// text blocks joined when the wire sent no raw output.
fn output_text(record: &ToolRecord) -> String {
    match &record.output {
        Some(value) => value_text(&payload_value(value)),
        None => record.text.join("\n"),
    }
}

/// Decode the host's preserved tool payload for display only.
fn payload_value(payload: &magistr_application::sessions::ToolPayload) -> Value {
    match payload {
        magistr_application::sessions::ToolPayload::Text(text) => Value::String(text.clone()),
        magistr_application::sessions::ToolPayload::Json(json) => {
            serde_json::from_str(json).unwrap_or_else(|_| Value::String(json.clone()))
        }
    }
}

/// A JSON value as the text a pane shows: a string itself, an array of
/// content blocks their texts joined, anything else pretty-printed.
fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        Value::Array(items) => items
            .iter()
            .map(|item| match item.get("text").and_then(Value::as_str) {
                Some(text) => text.to_owned(),
                None => value_text(item),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
    }
}

/// The first of `keys` the input object carries as a string.
fn field<'a>(input: Option<&'a Value>, keys: &[&str]) -> Option<&'a str> {
    let object = input?.as_object()?;
    keys.iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str))
}

/// `key` on the input object as a line number.
fn number(input: Option<&Value>, key: &str) -> Option<u32> {
    input?
        .get(key)?
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
}

/// The exit code a run's output object carried, when it carried one.
fn exit_code(record: &ToolRecord) -> Option<i64> {
    let value = payload_value(record.output.as_ref()?);
    let output = value.as_object()?;
    ["exit_code", "exitCode", "code"]
        .iter()
        .find_map(|key| output.get(*key).and_then(Value::as_i64))
}

/// An MCP tool's server and name off its `mcp__server__tool` title;
/// any other title is the tool, with no server.
fn mcp_name(title: &str) -> (Option<&str>, &str) {
    let mut parts = title.splitn(3, "__");
    match (parts.next(), parts.next(), parts.next()) {
        (Some("mcp"), Some(server), Some(tool)) => (Some(server), tool),
        _ => (None, title),
    }
}

/// How many cells the subject may take: the card's mono capacity less
/// the verb column, the measure and the gaps between them — so a long
/// command reads as elided rather than carried off the card's edge.
fn title_budget(model: &Model, word: &str, measure: usize) -> usize {
    let cols = usize::from(model.mono_cols().unwrap_or_else(|| model.turn_width()));
    // The icon and its gap are about two cells; each head gap about one.
    let taken = 2 + word.len() + 1 + measure + usize::from(measure > 0);
    cols.saturating_sub(taken).max(FeedComposition::TITLE_FLOOR)
}

/// `text` cut to `keep` characters, an ellipsis marking the cut.
fn cut(text: &str, keep: usize) -> String {
    if text.chars().count() <= keep {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(keep.saturating_sub(1)).collect();
    out.push('\u{2026}');
    out
}

/// An expanded edit card's `[Open] [Undo]` row, its ids indexed by the
/// row so two edits' buttons never collide.
fn buttons(at: usize, medium: Medium) -> Element {
    use crate::action::Action;
    use crate::action_row::ActionRow;
    let base = FeedComposition::BUTTONS + 2 * at as u64;
    ActionRow::new(
        None,
        vec![
            Action::new("Open", ElementId(base)),
            Action::new("Undo", ElementId(base + 1)),
        ],
    )
    .el(medium)
}

/// An image row's reservation: at the block's head, the placement's
/// whole footprint in the surface's own pixels; the rows under it are
/// already counted by the footprint, so they reserve nothing.
fn image_slot(model: &Model, at: usize, cell: (f32, f32)) -> Element {
    match model.turn().image_at_row(at) {
        Some((_, offset, (cols, rows))) if offset == 0 || at == model.top_row() => {
            let head = at.saturating_sub(usize::from(offset));
            Element::surface()
                .height(f32::from(rows.saturating_sub(offset)) * cell.1)
                .child(
                    Element::surface()
                        .width(f32::from(cols) * cell.0)
                        .height(f32::from(rows) * cell.1)
                        .pin_left(0.0)
                        .pin_top(-f32::from(offset) * cell.1)
                        .id(ElementId(FeedComposition::IMAGES + head as u64)),
                )
        }
        _ => Element::surface().height(0.0),
    }
}

#[cfg(test)]
#[path = "comp_feed.tests.rs"]
mod tests;
