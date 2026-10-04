use magistr_cockpit::cockpit::{FrameClock, Model, SignalRow, StateWord};
use qs_gpu::color::Srgba;

use crate::action::Action;
use crate::action_row::ActionRow;
use crate::card::Card;
use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::icon::Icon;
use crate::lexicon::Lexicon;
use crate::medium::Medium;
use crate::oklab;
use crate::pill::DataPill;
use crate::severity_cap::SeverityCap;
use crate::voice::Voice;

/// The Signals view, composed from the model: the stakeholder inbox as a column of
/// cards at the reading measure, each in the design's state vocabulary — **Open** on
/// the peach ground with Promote · Research · Snooze, **Researched** on the blue ground
/// with its findings and Promote lit in the accent, **Promoted** on the green ground,
/// settled, naming the star it became. The header borrows [`SignalsComposition::chips`]
/// for its right-hand slot: the open count with the peach dot.
///
/// Everything here is think-and-ship's real inbox as the cockpit's
/// [`Roadmap`](magistr_cockpit::cockpit::Roadmap) holds it — no second store. The verbs
/// route to the cockpit's own signal widgets by the signal's index in
/// [`Roadmap::signals`], the list `update` indexes, so a card and its button cannot
/// disagree about which signal they are. Motion: none — an inbox holds still until
/// something arrives.
///
/// [`Roadmap::signals`]: magistr_cockpit::cockpit::Roadmap::signals
pub struct SignalsComposition;

impl SignalsComposition {
    /// The id space a signal's \[Promote\] indexes into, by its index
    /// among the open signals.
    pub const PROMOTE: u64 = 1 << 23;
    /// The id space a signal's \[Research\] indexes into, likewise.
    pub const RESEARCH: u64 = 1 << 24;
    /// The id space a signal's \[Snooze\] indexes into, likewise.
    pub const SNOOZE: u64 = 1 << 25;
    /// The end of the snooze space.
    pub const SNOOZE_END: u64 = 1 << 26;

    /// The view's header band — the same `padding:15px 20px 12px 92px`
    /// `hair-b` row the Trace and the Galaxy draw, at the Galaxy's
    /// measure: no crumb glyph lifts this line the extra pixel the
    /// Trace's does, so the hairline sits on the capture's row 50 and
    /// the first card's edge on row 73.
    pub(crate) const HEADER_HEIGHT: f32 = 51.0;

    /// The body's margins (`padding:22px 34px`) and the column's
    /// measure (`max-width:620px`).
    const PAD_X: f32 = 34.0;
    const PAD_Y: f32 = 22.0;
    const MEASURE: f32 = 620.0;
    /// A card's padding (`.float7 { padding:14px 17px }`) and the air
    /// between cards (`margin-bottom:12px`).
    const CARD_PAD_X: f32 = 17.0;
    const CARD_PAD_Y: f32 = 14.0;
    const CARD_GAP: f32 = 12.0;
    /// The cap's line box. The volume sets the cap's glyph inline on the
    /// baseline (`.cap7 svg.ic`), and an 11px glyph on a 9.5px line
    /// stretches the box past the voice's own 14.25: measured on the
    /// capture, the title starts 16 below the cap's top. The text keeps
    /// its place at the top of the box; the box just runs on under it.
    const CAP_ROW: f32 = 16.0;
    /// The air under the cap (`margin-bottom:4px`), under the title
    /// (`4px`, `2px` on the settled card) and under the detail before
    /// the verbs (`10px`); the verbs' pitch (`gap:8px`).
    const CAP_BELOW: f32 = 4.0;
    const TITLE_BELOW: f32 = 4.0;
    const TITLE_BELOW_SETTLED: f32 = 2.0;
    const DETAIL_BELOW: f32 = 10.0;
    const VERBS_GAP: f32 = 8.0;
    /// How far the promoted card settles (`opacity:.7`).
    const SETTLED: f32 = 0.7;

    /// The card's title — the volume's inline sans 13.5 at the regular
    /// cut (`font-size:13.5px; font-weight:550`).
    const TITLE: Voice = Voice {
        family: "sans",
        px: 13.5,
        weight: 550,
        tracking_em: 0.0,
        line_height: 0.0,
        upper: false,
    };
    /// The detail line's voice — the volume's inline `mono 10.5px`,
    /// which the stakeholder's words and the promotion line share.
    const DETAIL: Voice = Voice {
        family: "mono",
        px: 10.5,
        weight: 400,
        tracking_em: 0.0,
        line_height: 0.0,
        upper: false,
    };
    /// The findings' voice — the volume's inline sans 12, which the
    /// dashed slot's copy shares.
    const FINDINGS: Voice = Voice::SLOT;

    /// How many signals are still in triage — the header chip's count,
    /// and the Sky's.
    #[must_use]
    pub fn open_count(model: &Model) -> usize {
        model.roadmap().signals().len()
    }

    /// The header's right-hand slot: the open count as the plain chip
    /// with its dot in the attention hue (`.chip7` with a peach
    /// `.dot7`), or the quiet dot when nothing is open.
    #[must_use]
    pub fn chips(model: &Model, medium: Medium) -> Element {
        let lex = Lexicon::of(medium);
        let open = Self::open_count(model);
        let copy = format!("{open} open");
        if open == 0 {
            DataPill::quiet(&copy, &lex)
        } else {
            DataPill::marked(&copy, lex.peach, &lex)
        }
    }

    /// The view's body for `model`, in `medium`'s lexicon. `clock` is
    /// the frame's, for the ages: a clock with no wall-clock reading
    /// draws the caps without one.
    #[must_use]
    pub fn el(model: &Model, medium: Medium, clock: &FrameClock) -> Element {
        let lex = Lexicon::of(medium);
        if model.selected().is_some() {
            let height = (f32::from(model.size().rows) * f32::from(model.cell_size().height)
                - Self::HEADER_HEIGHT)
                .max(1.0);
            return crate::comp_workspace_details::DetailsComposition::el(model, height, &lex)
                .grow(1.0);
        }
        let now = clock.epoch_secs().and_then(|secs| i64::try_from(secs).ok());
        let open = model.roadmap().signals();
        let mut column = Element::surface()
            .max_width(Self::MEASURE)
            .stretch()
            .gap(Self::CARD_GAP)
            .align_start();
        for signal in model.roadmap().inbox() {
            let Some(reading) = Reading::of(signal) else {
                continue;
            };
            let index = open.iter().position(|s| s.id == signal.id);
            column = column.child(card(model, signal, reading, index, now, medium, &lex).stretch());
        }
        Element::surface()
            .grow(1.0)
            .min_height(0.0)
            .pad_x(Self::PAD_X)
            .pad_y(Self::PAD_Y)
            .align_start()
            .child(column)
    }
}

/// The three states a card can be read in — derived from the wire
/// status, never flagged. A dismissed signal has no reading: the inbox
/// does not show it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reading {
    Open,
    Researched,
    Promoted,
}

impl Reading {
    fn of(signal: &SignalRow) -> Option<Self> {
        match signal.status.as_str() {
            "dismissed" => None,
            "promoted" => Some(Self::Promoted),
            "researched" | "surfaced" => Some(Self::Researched),
            _ => Some(Self::Open),
        }
    }

    /// The state's word, as the cap says it.
    const fn word(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Researched => "Researched",
            Self::Promoted => "Promoted",
        }
    }

    /// The state's hue: peach for attention, blue for the researched,
    /// green for the done.
    fn hue(self, lex: &Lexicon) -> Srgba {
        match self {
            Self::Open => lex.peach,
            Self::Researched => lex.blue,
            Self::Promoted => lex.green,
        }
    }

    /// The cap's glyph: the signal beacon while it is one, the check
    /// once it became a star.
    const fn icon(self) -> Icon {
        match self {
            Self::Open | Self::Researched => Icon::Signal,
            Self::Promoted => Icon::Done,
        }
    }
}

/// One signal's card: the cap (state, who, how long ago), the title,
/// the detail line and — while it is still in triage — its verbs.
fn card(
    model: &Model,
    signal: &SignalRow,
    reading: Reading,
    index: Option<usize>,
    now: Option<i64>,
    medium: Medium,
    lex: &Lexicon,
) -> Element {
    let settled = reading == Reading::Promoted;
    // `mix(a, b, share)` is CSS's `color-mix(a share%, b)`: the whole
    // card keeps its 70% share over the ground, ink and tint alike.
    let settle = |ink: Srgba| {
        if settled {
            oklab::mix(ink, lex.base, SignalsComposition::SETTLED)
        } else {
            ink
        }
    };
    let hue = reading.hue(lex);
    let mut cap_parts = vec![reading.word().to_owned()];
    if !settled && !signal.from.is_empty() {
        cap_parts.push(signal.from.clone());
    }
    if let Some(age) = now.and_then(|now| signal.age_text(now)) {
        cap_parts.push(age);
    }
    let cap = Element::row()
        .height(SignalsComposition::CAP_ROW)
        .align_start()
        .child(SeverityCap::tinted(
            settle(hue),
            Some(reading.icon()),
            &cap_parts.join(" \u{b7} "),
        ));
    let title = Element::row().stretch().child(
        Element::text(SignalsComposition::TITLE, settle(lex.tx), signal.title())
            .claim(Component::Title)
            .min_width(0.0),
    );
    let detail = match reading {
        Reading::Promoted => Some(
            Element::text(
                SignalsComposition::DETAIL,
                settle(lex.ov0),
                promoted_line(model, signal),
            )
            .claim(Component::Prose),
        ),
        Reading::Researched if signal.findings.is_some() => Some(
            Element::text(
                SignalsComposition::FINDINGS,
                lex.sb0,
                signal.findings.as_deref().unwrap_or_default(),
            )
            .claim(Component::Prose),
        ),
        // The stakeholder's own words under the summary that titles
        // the card; with no summary the words *are* the title.
        Reading::Open | Reading::Researched => signal.summary.is_some().then(|| {
            Element::text(SignalsComposition::DETAIL, lex.ov0, signal.body.as_str())
                .claim(Component::Prose)
        }),
    };
    let mut float = Card::tinted(
        settle(hue),
        SignalsComposition::CARD_PAD_X,
        SignalsComposition::CARD_PAD_Y,
        lex,
    )
    .align_start()
    .child(cap)
    .child(air(SignalsComposition::CAP_BELOW))
    .child(title);
    if settled {
        float = float.fill(oklab::mix(
            Card::tint(hue, lex),
            lex.base,
            SignalsComposition::SETTLED,
        ));
    }
    if let Some(detail) = detail {
        float = float
            .child(air(if settled {
                SignalsComposition::TITLE_BELOW_SETTLED
            } else {
                SignalsComposition::TITLE_BELOW
            }))
            .child(Element::row().stretch().child(detail.min_width(0.0)));
    }
    if let Some(index) = index.filter(|_| !settled) {
        let at = index as u64;
        let promote = Action::new("Promote", ElementId(SignalsComposition::PROMOTE + at));
        let mut verbs = vec![if reading == Reading::Researched {
            promote.accent()
        } else {
            promote
        }];
        if reading == Reading::Open {
            verbs.push(Action::new(
                "Research",
                ElementId(SignalsComposition::RESEARCH + at),
            ));
        }
        verbs.push(Action::new(
            "Snooze",
            ElementId(SignalsComposition::SNOOZE + at),
        ));
        float = float.child(air(SignalsComposition::DETAIL_BELOW)).child(
            ActionRow::new(None, verbs)
                .gap(SignalsComposition::VERBS_GAP)
                .el(medium),
        );
    }
    float
}

/// The promoted card's line: the star it became and that star's
/// status as the roadmap reads it now — `Promoted to: <chunk> (Done)`.
fn promoted_line(model: &Model, signal: &SignalRow) -> String {
    let Some(chunk) = signal.promoted_to.as_deref() else {
        return "Promoted".to_owned();
    };
    // The one state vocabulary: the graph's reading of the star when it draws one, the
    // wire status alone otherwise.
    match model.roadmap().chunk_for_ref(&format!("chunk:{chunk}")) {
        Some(star) => {
            let word = model.roadmap().star_state(&star.id).map_or_else(
                || StateWord::of_state(star.state, false),
                StateWord::of_star,
            );
            format!("Promoted to: {chunk} ({})", word.word())
        }
        None => format!("Promoted to: {chunk}"),
    }
}

/// A rigid run of vertical air.
fn air(px: f32) -> Element {
    Element::surface().height(px)
}

#[cfg(test)]
#[path = "comp_signals.tests.rs"]
mod tests;
