use magistr_cockpit::cockpit::{Model, TraceStep};
use qs_gpu::color::Srgba;

use crate::action::Action;
use crate::action_row::ActionRow;
use crate::card::Card;
use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::icon::Icon;
use crate::lane_stop::LaneStop;
use crate::lexicon::Lexicon;
use crate::mark::Mark;
use crate::medium::Medium;
use crate::oklab;
use crate::pill::DataPill;
use crate::receipt_row::ReceiptRow;
use crate::voice::Voice;

/// The Trace view, composed from the model: the think-step feed at reading width on the
/// left — a card per step with its number, its refs and, on the step the reader opened,
/// its verbs — and the BRANCHES rail on the right: the graph's reservation, the main /
/// branches receipt rows and the pinned list. The header borrows
/// [`TraceComposition::chips`] for its right-hand slot.
///
/// Everything here is think-and-ship's real trace as the cockpit's
/// [`Trace`](magistr_cockpit::cockpit::Trace) holds it — steps, pins, branches,
/// execution refs; no second store. The graph itself is not composed:
/// [`TraceGraph`](crate::trace_graph::TraceGraph) draws into the field this composition
/// reserves, from the same [`TraceComposition::lane`] the frame reads, so a dot and its
/// card cannot disagree about which step they are.
///
/// The design's two readings of a step are derived, never flagged: *pinned steps hold
/// their accent* (the figcaption's own law) — every other step but the live one recedes
/// at 75% — and the trace's newest step is *in progress* until it is final.
pub struct TraceComposition;

impl TraceComposition {
    /// The graph's field — a reservation the drawing fills.
    pub const GRAPH: ElementId = ElementId(340);
    /// The id space a step's card indexes into, by feed index.
    pub const CARDS: u64 = 1 << 20;
    /// The id space a step's \[Pin\]/\[Unpin\] indexes into, by step
    /// number.
    pub const PINS: u64 = 1 << 21;
    /// The id space a step's \[View receipts\] indexes into, by step
    /// number.
    pub const RECEIPTS: u64 = 1 << 22;
    /// The end of the receipts space — the next space is the frame's.
    pub const RECEIPTS_END: u64 = 1 << 23;

    /// The view's header band: the Galaxy's padding (`15px 20px 12px
    /// 92px`) over a taller line — the crumb's glyph and the chip lift
    /// it a pixel — its hairline on the capture's row 51.
    pub(crate) const HEADER_HEIGHT: f32 = 52.0;

    /// Main-line steps the feed shows — the newest, oldest first. The
    /// volume's window (view 11 shows three); the rest of the trace is
    /// the rail's reading until the feed scrolls.
    pub const TAIL: usize = 3;

    /// The feed's margins (`padding:18px 30px`) and its measure
    /// (`max-width:600px`).
    const FEED_PAD_Y: f32 = 18.0;
    const FEED_PAD_X: f32 = 30.0;
    const MEASURE: f32 = 600.0;
    /// A card's padding (`.float7 { padding:12px 15px }`) and the air
    /// between cards (`margin-bottom:10px`).
    const CARD_PAD_X: f32 = 15.0;
    const CARD_PAD_Y: f32 = 12.0;
    const CARD_GAP: f32 = 10.0;
    /// The head row's gap and the air under it (`gap:8px;
    /// margin-bottom:3px`), the verbs' air above and gap (`margin-top:9px;
    /// gap:6px`).
    const HEAD_GAP: f32 = 8.0;
    const HEAD_BELOW: f32 = 3.0;
    const VERBS_ABOVE: f32 = 9.0;
    const VERBS_GAP: f32 = 6.0;
    /// How far a quiet card recedes (`opacity:.75`).
    const QUIET: f32 = 0.75;
    /// The live step's caret (`.caret7 { width:1.5px; height:14px }`).
    const CARET: (f32, f32) = (1.5, 14.0);
    /// The cap's icon (`.cap7 svg.ic { width:11px }`) and its gap.
    const CAP_ICON_PX: u32 = 11;
    const CAP_GAP: f32 = 4.0;
    /// The rail (`width:280px; padding:18px 20px`, `hair-l`), the
    /// graph's box (`svg width="230" height="150"`), the air under the
    /// Branches cap (`margin-bottom:10px`) and around the Pinned cap
    /// (`margin:18px 0 8px`).
    pub(crate) const RAIL_WIDTH: f32 = 280.0;
    const RAIL_PAD_X: f32 = 20.0;
    const RAIL_PAD_Y: f32 = 18.0;
    const GRAPH_SIZE: (f32, f32) = (230.0, 150.0);
    /// The volume sets the graph's `<svg>` inline, so it sits on a line
    /// box: measured on the capture, the drawing starts 4 below the
    /// cap's 10 and the rows resume 1 under it.
    const CAP_BELOW: f32 = 14.0;
    const GRAPH_BELOW: f32 = 1.0;
    const PINNED_ABOVE: f32 = 18.0;
    const PINNED_BELOW: f32 = 8.0;
    /// Pinned steps the rail lists — the volume's three.
    const PINNED_ROWS: usize = 3;
    /// The header slot's pitch (`gap:14px`).
    const CHIP_GAP: f32 = 14.0;

    /// A quiet card's ref — the volume's inline `mono 9.5px`.
    const REF: Voice = Voice {
        family: "mono",
        px: 9.5,
        weight: 400,
        tracking_em: 0.0,
        line_height: 0.0,
        upper: false,
    };
    /// The step's own words — the volume's sans 12.5, which the Steer
    /// placeholder shares.
    const BODY: Voice = Voice::STEER;
    /// The pinned list's voice — `mono 10.5px; line-height:2`.
    const PINNED_LINE: Voice = Voice {
        family: "mono",
        px: 10.5,
        weight: 400,
        tracking_em: 0.0,
        line_height: 2.0,
        upper: false,
    };

    /// The main line: every step off a branch, with its feed index.
    #[must_use]
    pub fn main_steps(model: &Model) -> Vec<(usize, &TraceStep)> {
        model
            .trace()
            .steps()
            .iter()
            .enumerate()
            .filter(|(_, step)| step.branch_id.is_none())
            .collect()
    }

    /// The steps the feed shows: the newest [`TraceComposition::TAIL`]
    /// of the main line, oldest first.
    #[must_use]
    pub fn window(model: &Model) -> Vec<(usize, &TraceStep)> {
        let main = Self::main_steps(model);
        let from = main.len().saturating_sub(Self::TAIL);
        main[from..].to_vec()
    }

    /// The graph's stops, bottom first: the main step before the window
    /// as the lane's anchor, then the window's steps — the one the
    /// reader opened focused, the newest open while it is not final,
    /// and a fork on the step each branch left from.
    #[must_use]
    pub fn lane(model: &Model) -> Vec<LaneStop> {
        let trace = model.trace();
        let main = Self::main_steps(model);
        let from = main.len().saturating_sub(Self::TAIL);
        let anchor = from.checked_sub(1).map(|at| main[at]);
        let newest = main.last().map(|(_, step)| step.number);
        // Each branch's first step, and the main step it forked from.
        let mut seen: Vec<&str> = Vec::new();
        let forks: Vec<(u32, usize)> = trace
            .steps()
            .iter()
            .enumerate()
            .filter_map(|(index, step)| {
                let branch = step.branch_id.as_deref()?;
                if seen.contains(&branch) {
                    return None;
                }
                seen.push(branch);
                Some((step.branch_from?, index))
            })
            .collect();
        anchor
            .into_iter()
            .chain(main[from..].iter().copied())
            .map(|(index, step)| LaneStop {
                index,
                focused: trace.is_expanded(step.number),
                open: newest == Some(step.number) && !step.is_final,
                fork: forks
                    .iter()
                    .find(|(from, _)| *from == step.number)
                    .map(|(_, tip)| *tip),
            })
            .collect()
    }

    /// The header's right-hand slot: the `N steps · M pinned` chip and
    /// the `esc · Back` cap.
    #[must_use]
    pub fn chips(model: &Model, medium: Medium) -> Element {
        let lex = Lexicon::of(medium);
        let steps = Self::main_steps(model).len();
        let pinned = model
            .trace()
            .steps()
            .iter()
            .filter(|step| step.pinned)
            .count();
        Element::row()
            .gap(Self::CHIP_GAP)
            .center()
            .child(DataPill::num(
                &format!("{steps} steps \u{b7} {pinned} pinned"),
                &lex,
            ))
            .child(
                Element::text(Voice::CAP, lex.ov0, "esc \u{b7} Back")
                    .claim(Component::KeyHint)
                    .nowrap(),
            )
    }

    /// The view's body for `model`, in `medium`'s lexicon: the feed
    /// column and the rail.
    #[must_use]
    pub fn el(model: &Model, medium: Medium) -> Element {
        let lex = Lexicon::of(medium);
        Element::row()
            .grow(1.0)
            .min_height(0.0)
            .child(feed(model, medium, &lex))
            .child(rail(model, &lex))
    }
}

/// The feed column: the window's cards at the measure.
fn feed(model: &Model, medium: Medium, lex: &Lexicon) -> Element {
    let window = TraceComposition::window(model);
    let newest = window.last().map(|(_, step)| step.number);
    let mut column = Element::surface()
        .max_width(TraceComposition::MEASURE)
        .stretch()
        .gap(TraceComposition::CARD_GAP)
        .align_start();
    for (index, step) in window {
        let live = newest == Some(step.number) && !step.is_final;
        column = column.child(card(model, index, step, live, medium, lex).stretch());
    }
    Element::surface()
        .grow(1.0)
        .min_height(0.0)
        .pad_x(TraceComposition::FEED_PAD_X)
        .pad_y(TraceComposition::FEED_PAD_Y)
        .align_start()
        .child(column)
}

/// One step's card: the head row (number and state, the refs), the
/// step's words, and — on the opened step — its verbs. A quiet step
/// (neither pinned nor live nor opened) recedes.
fn card(
    model: &Model,
    index: usize,
    step: &TraceStep,
    live: bool,
    medium: Medium,
    lex: &Lexicon,
) -> Element {
    let opened = model.trace().is_expanded(step.number);
    let quiet = !step.pinned && !live && !opened;
    // `mix(a, b, share)` is CSS's `color-mix(a share%, b)`: the ink
    // keeps its 75% share over the ground.
    let recede = |ink: Srgba| {
        if quiet {
            oklab::mix(ink, lex.base, TraceComposition::QUIET)
        } else {
            ink
        }
    };
    let (cap_ink, cap_text, pin) = if step.pinned {
        (lex.mauve, format!("{} \u{b7} Pinned", step.number), true)
    } else if live {
        (
            lex.mauve,
            format!("{} \u{b7} In progress", step.number),
            false,
        )
    } else {
        (lex.ov0, step.number.to_string(), false)
    };
    let mut cap = Element::row().gap(TraceComposition::CAP_GAP).center();
    if pin {
        cap = cap.child(Element::mark(
            Mark::Icon(Icon::Pin),
            TraceComposition::CAP_ICON_PX,
            Lexicon::ink_bytes(cap_ink),
        ));
    }
    cap = cap.child(
        Element::text(Voice::CAP, recede(cap_ink), cap_text)
            .claim(Component::Cap)
            .nowrap(),
    );
    // The row spans the card so the refs sit at its right edge.
    let mut head = Element::row()
        .stretch()
        .gap(TraceComposition::HEAD_GAP)
        .center()
        .child(cap)
        .child(Element::row().grow(1.0));
    for reference in refs(model, step) {
        head = head.child(if opened {
            DataPill::tag(&reference, lex)
        } else {
            Element::text(TraceComposition::REF, recede(lex.ov0), reference)
                .claim(Component::Value)
                .nowrap()
        });
    }
    let body_ink = recede(if quiet { lex.sb0 } else { lex.sb1 });
    let words = if step.thought.is_empty() {
        step.purpose.as_str()
    } else {
        step.thought.as_str()
    };
    // The words in a row so the live caret can follow them; the run
    // gives up flex's automatic minimum (`min_width(0)`) so it wraps to
    // the card's measure instead of running off it.
    let mut body = Element::row().stretch().align_end().child(
        Element::text(TraceComposition::BODY, body_ink, words)
            .claim(Component::Prose)
            .min_width(0.0),
    );
    if live {
        body = body.child(
            Element::surface()
                .width(TraceComposition::CARET.0)
                .height(TraceComposition::CARET.1)
                .fill(lex.mauve),
        );
    }
    let mut float = Card::el(
        TraceComposition::CARD_PAD_X,
        TraceComposition::CARD_PAD_Y,
        lex,
    )
    .align_start()
    .id(ElementId(TraceComposition::CARDS + index as u64))
    .child(head)
    .child(Element::surface().height(TraceComposition::HEAD_BELOW))
    .child(body);
    if quiet {
        float = float.fill(oklab::mix(lex.mantle, lex.base, TraceComposition::QUIET));
    }
    if opened {
        let number = u64::from(step.number);
        float = float
            .child(Element::surface().height(TraceComposition::VERBS_ABOVE))
            .child(
                ActionRow::new(
                    None,
                    vec![
                        Action::new(
                            if step.pinned { "Unpin" } else { "Pin" },
                            ElementId(TraceComposition::PINS + number),
                        ),
                        Action::new(
                            "View receipts",
                            ElementId(TraceComposition::RECEIPTS + number),
                        ),
                    ],
                )
                .gap(TraceComposition::VERBS_GAP)
                .el(medium),
            );
    }
    float
}

/// A step's refs: its execution ref, and the chunk the roadmap resolves
/// it to when that is a different name — the same resolution the
/// \[View receipts\] jump follows, so the chip and the verb cannot point
/// two ways.
fn refs(model: &Model, step: &TraceStep) -> Vec<String> {
    let Some(reference) = step.execution_ref.as_deref() else {
        return Vec::new();
    };
    let mut refs = vec![reference.to_owned()];
    if let Some(chunk) = model.roadmap().chunk_for_ref(reference) {
        let named = format!("chunk:{}", chunk.id);
        if named != reference {
            refs.push(named);
        }
    }
    refs
}

/// The BRANCHES rail: the cap, the graph's field, the two receipt rows
/// and the pinned list, behind its left hairline.
fn rail(model: &Model, lex: &Lexicon) -> Element {
    let trace = model.trace();
    let inner = TraceComposition::RAIL_WIDTH - 1.0 - 2.0 * TraceComposition::RAIL_PAD_X;
    let main = TraceComposition::main_steps(model).len();
    // Every branch once, in the order the feed met it; the row reads
    // the count and the newest branch's status.
    let mut branches: Vec<&str> = Vec::new();
    for step in trace.steps() {
        if let Some(branch) = step.branch_id.as_deref()
            && !branches.contains(&branch)
        {
            branches.push(branch);
        }
    }
    let branches_value = match branches.last() {
        Some(newest) => format!("{} \u{b7} {}", branches.len(), trace.branch_status(newest)),
        None => "none".to_owned(),
    };
    let mut pinned: Vec<&TraceStep> = trace.steps().iter().filter(|step| step.pinned).collect();
    pinned.sort_by_key(|step| std::cmp::Reverse(step.number));
    let mut column = Element::surface()
        .width(TraceComposition::RAIL_WIDTH - 1.0)
        .pad_x(TraceComposition::RAIL_PAD_X)
        .pad_y(TraceComposition::RAIL_PAD_Y)
        .align_start()
        .child(cap(None, "Branches", lex))
        .child(air(TraceComposition::CAP_BELOW))
        .child(
            Element::surface()
                .width(TraceComposition::GRAPH_SIZE.0)
                .height(TraceComposition::GRAPH_SIZE.1)
                .id(TraceComposition::GRAPH),
        )
        .child(air(TraceComposition::GRAPH_BELOW))
        .child(
            Element::surface()
                .width(inner)
                .child(ReceiptRow::value(
                    (Icon::Roadmap, "main"),
                    &format!("{main} steps"),
                    None,
                    true,
                    lex.base,
                    lex,
                ))
                .child(ReceiptRow::value(
                    (Icon::Clone, "branches"),
                    &branches_value,
                    None,
                    true,
                    lex.base,
                    lex,
                )),
        )
        .child(air(TraceComposition::PINNED_ABOVE))
        .child(cap(Some(Icon::Pin), "Pinned", lex))
        .child(air(TraceComposition::PINNED_BELOW));
    if pinned.is_empty() {
        column = column.child(
            Element::text(Voice::SLOT, lex.sb0, "No pins yet")
                .claim(Component::Slot)
                .nowrap(),
        );
    }
    for step in pinned.iter().take(TraceComposition::PINNED_ROWS) {
        let words = if step.purpose.is_empty() {
            "step"
        } else {
            step.purpose.as_str()
        };
        column = column.child(
            Element::text(
                TraceComposition::PINNED_LINE,
                lex.sb0,
                format!("{} \u{b7} {words}", step.number),
            )
            .claim(Component::ListRow)
            .nowrap(),
        );
    }
    Element::row()
        .width(TraceComposition::RAIL_WIDTH)
        .no_shrink()
        .child(
            Element::surface()
                .width(1.0)
                .stretch()
                .fill(lex.grounded(lex.hairc)),
        )
        .child(column)
}

/// A section cap in the volume's own voice, with its glyph when it
/// wears one.
fn cap(icon: Option<Icon>, text: &str, lex: &Lexicon) -> Element {
    let mut row = Element::row().gap(TraceComposition::CAP_GAP).center();
    if let Some(icon) = icon {
        row = row.child(Element::mark(
            Mark::Icon(icon),
            TraceComposition::CAP_ICON_PX,
            Lexicon::ink_bytes(lex.ov0),
        ));
    }
    row.child(
        Element::text(Voice::CAP, lex.ov0, text)
            .claim(Component::Cap)
            .nowrap(),
    )
}

/// A rigid run of vertical air.
fn air(px: f32) -> Element {
    Element::surface().height(px)
}

#[cfg(test)]
#[path = "comp_trace.tests.rs"]
mod tests;
