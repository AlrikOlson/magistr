use magistr_cockpit::cockpit::{BirthMoment, Interview, Model, Proposal, SurveyReport};
use qs_gpu::color::Srgba;

use crate::action::Action;
use crate::action_row::ActionRow;
use crate::answer_field::AnswerField;
use crate::card::Card;
use crate::chart::Chart;
use crate::comp_sky::SkyComposition;
use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::finding_row::FindingRow;
use crate::icon::Icon;
use crate::interned::interned;
use crate::key_hint::KeyHint;
use crate::lexicon::Lexicon;
use crate::mark::Mark;
use crate::medium::Medium;
use crate::pick_chip::PickChip;
use crate::pill::DataPill;
use crate::slot::Slot;
use crate::voice::Voice;

/// Birth, composed from the model: the five moments a project is born through, each its
/// own arrangement over the one [`Birth`] the model keeps. F1 the ground card over the
/// Galaxy — the dashed slot, the two raised alternatives, `esc · Cancel`. F2 the survey
/// — the Detected column landing its finding rows, the Files written receipts, the scan
/// meter, all over the field the frame draws the sweep into. F3 the interview — the
/// answered cards stacked above the question being asked, its pick chips and own-answer
/// field, Continue. F4 the charting — the proposal's list beside the field the frame
/// draws the constellation into, the labels pinned from the same placement, the three
/// hints. F5 first light — the Sky's own field with the accepted roadmap, the Ready
/// star's label, the toast.
///
/// Every string here is derived from the flow's own data: the rows are the survey's,
/// the chips the interview's, the list the proposal's, the toast's line the written
/// roadmap's — the composition writes none of them.
///
/// [`Birth`]: magistr_cockpit::cockpit::Birth
pub struct BirthComposition;

impl BirthComposition {
    /// The ground card's slot wrapper — the drop target's id.
    pub const SLOT: ElementId = ElementId(380);
    /// `Clone a repository…`.
    pub const CLONE: ElementId = ElementId(381);
    /// `Start empty`.
    pub const EMPTY: ElementId = ElementId(382);
    /// `Continue ↵`.
    pub const CONTINUE: ElementId = ElementId(383);
    /// `Accept this roadmap`.
    pub const ACCEPT: ElementId = ElementId(384);
    /// `Redraw`.
    pub const REDRAW: ElementId = ElementId(385);
    /// `Edit chunks…`.
    pub const EDIT: ElementId = ElementId(386);
    /// `Claim <chunk>`.
    pub const CLAIM: ElementId = ElementId(387);
    /// `Later`.
    pub const LATER: ElementId = ElementId(388);
    /// The field the survey's sweep and the charting's constellation
    /// are drawn into — a reservation, not a drawing.
    pub const FIELD: ElementId = ElementId(389);
    /// The typed field on the ground card and the interview's
    /// own-answer field — visual state only.
    pub const ANSWER: ElementId = ElementId(390);
    /// The id space an interview chip indexes into — above the search
    /// and settings spaces, which end at `1 << 30`.
    pub const CHIPS: u64 = 1 << 30;
    /// The id space a proposal row indexes into.
    pub const ROWS: u64 = 1 << 31;
    /// The end of the rows' space.
    pub const ROWS_END: u64 = 1 << 32;

    /// The header band the survey, the interview and first light draw
    /// — the record band (`padding:15px 20px 12px 92px`).
    pub(crate) const HEADER_HEIGHT: f32 = 51.0;
    /// The charting's band, taller by its buttons — the Report's.
    pub(crate) const VERBS_HEADER_HEIGHT: f32 = 60.0;
    /// The header verbs' pitch (`gap:14px`).
    const VERBS_GAP: f32 = 14.0;

    /// F1's card: `width:520px; padding:24px 26px`, the title's air
    /// (4), the sub's (18), the slot's (14), the verbs' gap (10).
    const GROUND_WIDTH: f32 = 520.0;
    const GROUND_PAD_X: f32 = 26.0;
    const GROUND_PAD_Y: f32 = 24.0;
    const TITLE_BELOW: f32 = 4.0;
    const SUB_BELOW: f32 = 18.0;
    const SLOT_BELOW: f32 = 14.0;
    const GROUND_GAP: f32 = 10.0;
    /// The armed slot's ring — the accent at 1px around the wrapper.
    const ARMED_RING: f32 = 1.0;
    const ARMED_RADIUS: f32 = 12.0;

    /// F2's columns: `left:56px; top:44px; width:330px` and its mirror,
    /// the cap's air (10), the rows' pitch (8), the meter
    /// (`width:300px; bottom:30px`) and its label air (5).
    const SURVEY_INSET_X: f32 = 56.0;
    const SURVEY_INSET_Y: f32 = 44.0;
    const SURVEY_COLUMN: f32 = 330.0;
    const SURVEY_CAP_BELOW: f32 = 10.0;
    const SURVEY_ROW_GAP: f32 = 8.0;
    const METER_WIDTH: f32 = 300.0;
    const METER_BOTTOM: f32 = 30.0;
    const METER_LABEL_BELOW: f32 = 5.0;
    const METER_TRACK: f32 = 4.0;
    /// A receipt row (`.rr { gap:10px; padding:6px 2px }`, key 58).
    const RECEIPT_GAP: f32 = 10.0;
    const RECEIPT_PAD_X: f32 = 2.0;
    const RECEIPT_PAD_Y: f32 = 6.0;
    const RECEIPT_KEY: f32 = 58.0;
    const RECEIPT_KEY_GAP: f32 = 5.0;
    const RECEIPT_ICON_PX: u32 = 13;
    const RULE_SHARE: f32 = 0.6;

    /// F3's column (`width:560px`, cards `padding:20px 24px`, pitch
    /// 12), the cap's air (8), the prompt's (10), the chips' (12), the
    /// chips' pitch (8), the answered card's own air (4).
    const INTERVIEW_WIDTH: f32 = 560.0;
    const CARD_PAD_X: f32 = 24.0;
    const CARD_PAD_Y: f32 = 20.0;
    const CARD_GAP: f32 = 12.0;
    const CAP_BELOW: f32 = 8.0;
    const PROMPT_BELOW: f32 = 10.0;
    const CHIPS_BELOW: f32 = 12.0;
    const CHIP_GAP: f32 = 8.0;
    const ANSWER_ABOVE: f32 = 4.0;
    /// The volume seats the column's *centre* at `top:47%`
    /// (`translate(-50%,-50%)`); the room's leftover air is split
    /// above and below in the share that puts a column of the
    /// volume's own height there.
    const INTERVIEW_ABOVE: f32 = 0.42;

    /// F4's list pane (`width:300px; padding:16px 0`), the caps'
    /// padding (`0 20px 10px` / `16px 20px 6px`), a row's line
    /// (`line-height:2.15` on 11px) and padding, the hints' foot
    /// (`bottom:22px; gap:18px`), a label's offset from its star.
    const LIST_WIDTH: f32 = 300.0;
    const LIST_PAD_Y: f32 = 16.0;
    const LIST_PAD_X: f32 = 20.0;
    const LIST_CAP_BELOW: f32 = 10.0;
    const SOURCES_ABOVE: f32 = 16.0;
    const SOURCES_BELOW: f32 = 6.0;
    const ROW_PAD_Y: f32 = 1.0;
    const HINTS_BOTTOM: f32 = 22.0;
    const HINTS_GAP: f32 = 18.0;
    const LABEL_DX: f32 = -50.0;
    const LABEL_DY: f32 = -28.0;
    /// Labels the charting pins: the first star, one in the middle, the
    /// last — the volume names three of eight.
    const LABELS: usize = 3;

    /// F5's toast (`right:22px; bottom:22px; padding:14px 17px;
    /// width:290px`), its airs (4, 2, 10) and its verbs' gap (8); the
    /// Ready label's seat (`left:24%; top:35%`).
    const TOAST_INSET: f32 = 22.0;
    const TOAST_WIDTH: f32 = 290.0;
    const TOAST_PAD_X: f32 = 17.0;
    const TOAST_PAD_Y: f32 = 14.0;
    const TOAST_CAP_BELOW: f32 = 4.0;
    const TOAST_NAME_BELOW: f32 = 2.0;
    const TOAST_LINE_BELOW: f32 = 10.0;
    const TOAST_GAP: f32 = 8.0;
    const READY_LEFT: f32 = 0.24;
    const READY_TOP: f32 = 0.35;
    /// The sigil in a card's name row (`.ic.sig` 14px) and its gap.
    const SIGIL_PX: u32 = 14;
    const SIGIL_GAP: f32 = 6.0;

    /// F1's title (`font-size:16px; font-weight:600`).
    const TITLE: Voice = Voice {
        family: "sans",
        px: 16.0,
        weight: 600,
        tracking_em: 0.0,
        line_height: 0.0,
        upper: false,
    };
    /// A question's prompt (`font-size:14.5px; font-weight:600`).
    const PROMPT: Voice = Voice {
        family: "sans",
        px: 14.5,
        weight: 600,
        tracking_em: 0.0,
        line_height: 0.0,
        upper: false,
    };
    /// An answered prompt (`font-size:13px`) and its answer (13.5).
    const ASKED: Voice = Voice {
        family: "sans",
        px: 13.0,
        weight: 400,
        tracking_em: 0.0,
        line_height: 0.0,
        upper: false,
    };
    const ANSWERED: Voice = Voice {
        family: "sans",
        px: 13.5,
        weight: 400,
        tracking_em: 0.0,
        line_height: 0.0,
        upper: false,
    };
    /// A proposal row (`.mono` 11 on 2.15).
    const ROW: Voice = Voice {
        family: "mono",
        px: 11.0,
        weight: 400,
        tracking_em: 0.0,
        line_height: 2.15,
        upper: false,
    };
    /// The sources' lines (`.mono` 10 on 1.8).
    const SOURCES: Voice = Voice {
        family: "mono",
        px: 10.0,
        weight: 400,
        tracking_em: 0.0,
        line_height: 1.8,
        upper: false,
    };
    /// A star's label (`.mono` 10).
    const LABEL: Voice = Voice {
        family: "mono",
        px: 10.0,
        weight: 400,
        tracking_em: 0.0,
        line_height: 0.0,
        upper: false,
    };
    /// The toast's name (`font-size:13px; font-weight:600`).
    const NAME: Voice = Voice {
        family: "sans",
        px: 13.0,
        weight: 600,
        tracking_em: 0.0,
        line_height: 0.0,
        upper: false,
    };

    /// Whether the moment draws under the header band — every moment
    /// but the ground, which floats over the Galaxy with no path.
    #[must_use]
    pub fn has_header(model: &Model) -> bool {
        model.birth().moment() != BirthMoment::Ground
    }

    /// The header band's height for the moment: the charting's carries
    /// buttons and is the Report's; the rest are the record band's.
    #[must_use]
    pub fn header_height(model: &Model) -> f32 {
        if model.birth().moment() == BirthMoment::Charting {
            Self::VERBS_HEADER_HEIGHT
        } else {
            Self::HEADER_HEIGHT
        }
    }

    /// The header's right-hand slot, by the moment: `Scanning…` with
    /// the accent's dot on the survey, `N of 3` on the interview, the
    /// three verbs on the charting, `0 / N complete` at first light.
    #[must_use]
    pub fn chips(model: &Model, medium: Medium) -> Element {
        let lex = Lexicon::of(medium);
        let birth = model.birth();
        match birth.moment() {
            BirthMoment::Ground | BirthMoment::Survey => {
                DataPill::marked("Scanning\u{2026}", lex.mauve, &lex)
            }
            BirthMoment::Interview => {
                let (n, of) = birth
                    .interview()
                    .map_or((1, 3), |i| (i.current() + 1, i.len()));
                Element::text(Voice::CAP, lex.ov0, format!("{n} of {of}"))
                    .claim(Component::Counter)
                    .nowrap()
            }
            BirthMoment::Charting => ActionRow::new(
                Some(Action::new("Accept this roadmap", Self::ACCEPT)),
                vec![
                    Action::new("Redraw", Self::REDRAW),
                    Action::new("Edit chunks\u{2026}", Self::EDIT),
                ],
            )
            .gap(Self::VERBS_GAP)
            .el(medium),
            BirthMoment::FirstLight => {
                let total = birth.born_data().map_or(0, |data| data.chunks.len());
                Element::text(Voice::METER_VALUE, lex.ov0, format!("0 / {total} complete"))
                    .claim(Component::Meter)
                    .nowrap()
            }
        }
    }

    /// The moment's body for `model`, in `medium`'s lexicon, over a
    /// room `size` logical pixels big — what the root lays under the
    /// header (or, on the ground, over the whole window).
    #[must_use]
    pub fn el(model: &Model, medium: Medium, size: (f32, f32)) -> Element {
        let lex = Lexicon::of(medium);
        match model.birth().moment() {
            BirthMoment::Ground => ground(model, medium, &lex),
            BirthMoment::Survey => survey(model, &lex),
            BirthMoment::Interview => interview(model, medium, &lex),
            BirthMoment::Charting => charting(model, medium, size, &lex),
            BirthMoment::FirstLight => first_light(model, medium, size, &lex),
        }
    }

    /// The name of the star at `index`, as the label the charting pins
    /// reads it.
    fn labelled(proposal: &Proposal, index: usize) -> bool {
        let n = proposal.chunks.len();
        if n == 0 {
            return false;
        }
        let picks = [0, n / 2, n - 1];
        picks.iter().take(Self::LABELS).any(|&i| i == index)
    }
}

/// F1 — the ground card over the room: the title, the sub, the slot
/// (armed under a desktop drag, or the typed field while one of the
/// three ways in is being typed), the two alternatives and `esc ·
/// Cancel`.
fn ground(model: &Model, medium: Medium, lex: &Lexicon) -> Element {
    let birth = model.birth();
    let inner = BirthComposition::GROUND_WIDTH - 2.0 * BirthComposition::GROUND_PAD_X;
    let slot = match birth.field() {
        Some((field, text)) => {
            AnswerField::el(text, field.placeholder(), BirthComposition::ANSWER, lex).stretch()
        }
        None => {
            let copy = if birth.is_armed() {
                "Drop it here"
            } else {
                "Drop a folder here"
            };
            Slot::empty_state(Icon::Folder, copy, "Or click to browse", lex).stretch()
        }
    };
    let mut wrapper = Element::surface()
        .width(inner)
        .id(BirthComposition::SLOT)
        .focusable()
        .focus_ring(lex.mauve)
        .radius(BirthComposition::ARMED_RADIUS)
        .child(slot);
    if birth.is_armed() {
        wrapper = wrapper.stroke(lex.mauve, BirthComposition::ARMED_RING);
    }
    let card = Card::overlay(
        lex.base,
        BirthComposition::GROUND_PAD_X,
        BirthComposition::GROUND_PAD_Y,
        lex,
    )
    .width(BirthComposition::GROUND_WIDTH)
    .align_start()
    .child(
        Element::text(BirthComposition::TITLE, lex.tx, "New project")
            .claim(Component::Title)
            .nowrap(),
    )
    .child(air(BirthComposition::TITLE_BELOW))
    .child(
        Element::text(
            Voice::METER_VALUE,
            lex.ov0,
            "Select a folder, a repository, or start empty",
        )
        .claim(Component::Prose)
        .nowrap(),
    )
    .child(air(BirthComposition::SUB_BELOW))
    .child(wrapper)
    .child(air(BirthComposition::SLOT_BELOW))
    .child(
        Element::row()
            .width(inner)
            .gap(BirthComposition::GROUND_GAP)
            .center()
            .child(
                ActionRow::new(
                    None,
                    vec![
                        Action::new("Clone a repository\u{2026}", BirthComposition::CLONE),
                        Action::new("Start empty", BirthComposition::EMPTY),
                    ],
                )
                .gap(BirthComposition::GROUND_GAP)
                .el(medium),
            )
            .child(Element::row().grow(1.0))
            .child(
                Element::text(Voice::CAP, lex.ov0, "esc \u{b7} Cancel")
                    .claim(Component::KeyHint)
                    .nowrap(),
            ),
    );
    Element::surface()
        .grow(1.0)
        .min_height(0.0)
        .center()
        .justify_center()
        .child(card)
}

/// F2 — the survey: the field the sweep is drawn into, the Detected
/// column landing its rows, the Files written receipts, the scan meter.
fn survey(model: &Model, lex: &Lexicon) -> Element {
    let birth = model.birth();
    let animate = model.theme().motion().is_full();
    let now = model.now();
    let landed = birth.rows_landed(now, animate);
    let empty = SurveyReport::default();
    let report = birth.survey().unwrap_or(&empty);
    let mut detected = Element::surface()
        .width(BirthComposition::SURVEY_COLUMN)
        .align_start()
        .gap(BirthComposition::SURVEY_ROW_GAP)
        .child(
            Element::text(Voice::CAP, lex.ov0, "Detected")
                .claim(Component::Cap)
                .nowrap(),
        )
        .child(air(
            BirthComposition::SURVEY_CAP_BELOW - BirthComposition::SURVEY_ROW_GAP
        ));
    for (index, (kind, fact)) in report.detected.iter().enumerate().take(landed) {
        let row = FindingRow::el(
            survey_icon(*kind),
            kind.label(),
            fact,
            BirthComposition::SURVEY_COLUMN,
            lex,
        );
        detected = detected.child(risen(row, birth.row_rise(index, now, animate)));
    }
    let facts = report.detected.len();
    let mut written = Element::surface()
        .width(BirthComposition::SURVEY_COLUMN)
        .align_start()
        .child(
            Element::row()
                .center()
                .child(Element::mark(
                    Mark::Icon(Icon::Report),
                    BirthComposition::RECEIPT_ICON_PX - 2,
                    Lexicon::ink_bytes(lex.ov0),
                ))
                .child(
                    Element::text(Voice::CAP, lex.ov0, " Files written")
                        .claim(Component::Cap)
                        .nowrap(),
                ),
        )
        .child(air(BirthComposition::SURVEY_CAP_BELOW));
    let receipts = landed.saturating_sub(facts);
    let last = report.written.len().saturating_sub(1);
    for (index, (kind, value, there)) in report.written.iter().enumerate().take(receipts) {
        let verdict = if *there { kind.verdict() } else { "Skipped" };
        let ink = if *there { lex.green } else { lex.ov0 };
        let row = receipt(
            scaffold_icon(*kind),
            kind.label(),
            value,
            verdict,
            ink,
            index < last,
            lex,
        );
        written = written.child(risen(row, birth.row_rise(facts + index, now, animate)));
    }
    let (done, total) = birth.scan(now, animate);
    let fill = if total == 0 {
        0.0
    } else {
        done as f32 / total as f32
    };
    let meter = Element::surface()
        .width(BirthComposition::METER_WIDTH)
        .child(
            Element::row()
                .space_between()
                .center()
                .child(
                    Element::text(Voice::CAP, lex.ov0, "Scan")
                        .claim(Component::Cap)
                        .nowrap(),
                )
                .child(
                    Element::text(Voice::METER_VALUE, lex.sb1, format!("{done} / {total}"))
                        .claim(Component::Meter)
                        .nowrap(),
                ),
        )
        .child(air(BirthComposition::METER_LABEL_BELOW))
        .child(
            Element::row()
                .width(BirthComposition::METER_WIDTH)
                .height(BirthComposition::METER_TRACK)
                .radius(BirthComposition::METER_TRACK / 2.0)
                .fill(lex.s0)
                .child(
                    Element::surface()
                        .width(BirthComposition::METER_WIDTH * fill)
                        .radius(BirthComposition::METER_TRACK / 2.0)
                        .fill(lex.mauve)
                        .pin_left(0.0)
                        .pin_top(0.0)
                        .pin_bottom(0.0),
                ),
        );
    Element::surface()
        .grow(1.0)
        .min_height(0.0)
        .child(
            Element::surface()
                .pin_left(0.0)
                .pin_top(0.0)
                .pin_right(0.0)
                .pin_bottom(0.0)
                .id(BirthComposition::FIELD),
        )
        .child(
            detected
                .pin_left(BirthComposition::SURVEY_INSET_X)
                .pin_top(BirthComposition::SURVEY_INSET_Y),
        )
        .child(
            written
                .pin_right(BirthComposition::SURVEY_INSET_X)
                .pin_top(BirthComposition::SURVEY_INSET_Y),
        )
        .child(
            Element::row()
                .pin_left(0.0)
                .pin_right(0.0)
                .pin_bottom(BirthComposition::METER_BOTTOM)
                .justify_center()
                .child(meter),
        )
}

/// F3 — the interview: the answered cards stacked above the question
/// being asked, its chips and own-answer field, Continue and the
/// `Next:` cap.
fn interview(model: &Model, medium: Medium, lex: &Lexicon) -> Element {
    let birth = model.birth();
    let empty = SurveyReport::default();
    let fallback = Interview::of(&empty);
    let interview = birth.interview().unwrap_or(&fallback);
    let width = BirthComposition::INTERVIEW_WIDTH;
    let inner = width - 2.0 * BirthComposition::CARD_PAD_X;
    let mut column = Element::surface()
        .width(width)
        .align_start()
        .gap(BirthComposition::CARD_GAP);
    for question in interview.answered() {
        let answer = question.answer().unwrap_or_default();
        column = column.child(
            Card::el(
                BirthComposition::CARD_PAD_X,
                BirthComposition::CARD_PAD_Y,
                lex,
            )
            .width(width)
            .align_start()
            .child(
                Element::text(Voice::CAP, lex.ov0, "Answered")
                    .claim(Component::Cap)
                    .nowrap(),
            )
            .child(air(BirthComposition::CAP_BELOW))
            .child(
                Element::text(BirthComposition::ASKED, lex.sb0, question.prompt())
                    .claim(Component::Prose)
                    .width(inner),
            )
            .child(air(BirthComposition::ANSWER_ABOVE))
            .child(
                Element::text(
                    BirthComposition::ANSWERED,
                    lex.tx,
                    format!("\u{201c}{answer}\u{201d}"),
                )
                .claim(Component::Prose)
                .width(inner),
            ),
        );
    }
    if let Some(question) = interview.asking() {
        let n = interview.current() + 1;
        let of = interview.len();
        let mut chips = Element::row()
            .gap(BirthComposition::CHIP_GAP)
            .wrap()
            .width(inner);
        for (index, (label, picked)) in question.chips().into_iter().enumerate() {
            chips = chips.child(PickChip::el(
                label,
                picked,
                lex.mantle,
                ElementId(BirthComposition::CHIPS + index as u64),
                lex,
            ));
        }
        let mut card = Card::el(
            BirthComposition::CARD_PAD_X,
            BirthComposition::CARD_PAD_Y,
            lex,
        )
        .width(width)
        .align_start()
        .child(
            Element::text(Voice::CAP, lex.mauve, format!("Question {n} of {of}"))
                .claim(Component::Counter)
                .nowrap(),
        )
        .child(air(BirthComposition::CAP_BELOW))
        .child(
            Element::text(BirthComposition::PROMPT, lex.tx, question.prompt())
                .claim(Component::Prose)
                .width(inner),
        )
        .child(air(BirthComposition::PROMPT_BELOW));
        if !question.chips().is_empty() {
            card = card.child(chips).child(air(BirthComposition::CHIPS_BELOW));
        }
        card = card.child(
            AnswerField::el(
                question.own(),
                "Type your own answer",
                BirthComposition::ANSWER,
                lex,
            )
            .width(inner),
        );
        column = column.child(card);
        let next = match interview.next_short() {
            Some(short) => format!("Next: {short} \u{b7} {} remaining", interview.remaining()),
            None => "Last question".to_owned(),
        };
        column = column.child(
            Element::row()
                .width(width)
                .gap(BirthComposition::GROUND_GAP)
                .center()
                .child(
                    ActionRow::new(
                        None,
                        vec![
                            Action::new("Continue", BirthComposition::CONTINUE)
                                .accent()
                                .cap("\u{21b5}"),
                        ],
                    )
                    .el(medium),
                )
                .child(Element::row().grow(1.0))
                .child(
                    Element::text(Voice::CAP, lex.ov0, next)
                        .claim(Component::Cap)
                        .nowrap(),
                ),
        );
    }
    // The column's centre sits at 47% of the room.
    let above = BirthComposition::INTERVIEW_ABOVE;
    Element::surface()
        .grow(1.0)
        .min_height(0.0)
        .center()
        .child(Element::surface().grow(above))
        .child(column)
        .child(Element::surface().grow(1.0 - above))
}

/// F4 — the charting: the proposal's list beside the field the
/// constellation is drawn into, the labels pinned from the same
/// placement, the three hints.
fn charting(model: &Model, medium: Medium, size: (f32, f32), lex: &Lexicon) -> Element {
    let birth = model.birth();
    let empty = Proposal::default();
    let proposal = birth.proposal().unwrap_or(&empty);
    let animate = model.theme().motion().is_full();
    let now = model.now();
    let mut list = Element::surface()
        .width(BirthComposition::LIST_WIDTH)
        .pad_y(BirthComposition::LIST_PAD_Y)
        .align_start()
        .child(
            Element::row().pad_x(BirthComposition::LIST_PAD_X).child(
                Element::text(
                    Voice::CAP,
                    lex.ov0,
                    format!("Proposed \u{b7} {}", proposal.summary()),
                )
                .claim(Component::Cap)
                .nowrap(),
            ),
        )
        .child(air(BirthComposition::LIST_CAP_BELOW));
    let editing = birth.editing();
    let dragging = birth.dragging();
    for (index, chunk) in proposal.chunks.iter().enumerate() {
        let hue = Chart::hue_in(lex, proposal.hue_of(&chunk.group));
        let copy = match editing {
            Some((at, text)) if at == index => format!("{} \u{b7} {text}\u{2502}", chunk.group),
            _ => chunk.row(),
        };
        let ink = if editing.is_some_and(|(at, _)| at == index) {
            lex.mauve
        } else {
            lex.sb0
        };
        let mut row = Element::row()
            .pad_x(BirthComposition::LIST_PAD_X)
            .pad_y(BirthComposition::ROW_PAD_Y)
            .gap(BirthComposition::SIGIL_GAP)
            .center()
            .id(ElementId(BirthComposition::ROWS + index as u64))
            .focusable()
            .focus_ring(lex.mauve)
            .hover_fill(lex.mantle)
            .child(
                Element::text(BirthComposition::ROW, hue, "\u{25cf}")
                    .claim(Component::ListRow)
                    .nowrap(),
            )
            .child(
                Element::text(BirthComposition::ROW, ink, copy)
                    .claim(Component::ListRow)
                    .nowrap(),
            );
        if dragging == Some(index) {
            row = row.fill(lex.mantle);
        }
        list = list.child(row);
    }
    list = list
        .child(air(BirthComposition::SOURCES_ABOVE))
        .child(
            Element::row().pad_x(BirthComposition::LIST_PAD_X).child(
                Element::text(Voice::CAP, lex.ov0, "Sources")
                    .claim(Component::Cap)
                    .nowrap(),
            ),
        )
        .child(air(BirthComposition::SOURCES_BELOW));
    for source in &proposal.sources {
        list = list.child(
            Element::row().pad_x(BirthComposition::LIST_PAD_X).child(
                Element::text(BirthComposition::SOURCES, lex.ov0, source)
                    .claim(Component::ListRow)
                    .nowrap(),
            ),
        );
    }
    // The field: the frame draws the constellation; the labels are
    // pinned here from the same placement, each at its star's alpha —
    // a label is born with its star.
    let pane = (
        (size.0 - BirthComposition::LIST_WIDTH - 1.0).max(0.0),
        size.1.max(0.0),
    );
    let placed = Chart::place(proposal, pane);
    let mut field = Element::surface().grow(1.0).min_height(0.0).child(
        Element::surface()
            .pin_left(0.0)
            .pin_top(0.0)
            .pin_right(0.0)
            .pin_bottom(0.0)
            .id(BirthComposition::FIELD),
    );
    for (index, (chunk, (x, y))) in proposal.chunks.iter().zip(&placed).enumerate() {
        if !BirthComposition::labelled(proposal, index) {
            continue;
        }
        let alpha = birth.born_alpha(index, now, animate);
        if alpha <= 0.0 {
            continue;
        }
        let ink = Srgba::new(lex.sb0.r, lex.sb0.g, lex.sb0.b, alpha);
        field = field.child(
            Element::text(BirthComposition::LABEL, ink, chunk.name.as_str())
                .claim(Component::Label)
                .nowrap()
                .pin_left(x + BirthComposition::LABEL_DX)
                .pin_top(y + BirthComposition::LABEL_DY),
        );
    }
    field = field.child(
        Element::row()
            .pin_left(0.0)
            .pin_right(0.0)
            .pin_bottom(BirthComposition::HINTS_BOTTOM)
            .justify_center()
            .gap(BirthComposition::HINTS_GAP)
            .child(KeyHint::el("drag", "Reorder", medium, lex.base, lex))
            .child(KeyHint::el("right-click", "Edit", medium, lex.base, lex))
            .child(KeyHint::el("\u{21b5}", "Accept", medium, lex.base, lex)),
    );
    Element::row()
        .grow(1.0)
        .min_height(0.0)
        .child(list)
        .child(
            Element::surface()
                .width(1.0)
                .stretch()
                .fill(lex.grounded(lex.hairc)),
        )
        .child(field)
}

/// F5 — first light: the Sky's own field with the accepted roadmap,
/// the Ready star's label, the toast with Claim and Later.
fn first_light(model: &Model, medium: Medium, size: (f32, f32), lex: &Lexicon) -> Element {
    let birth = model.birth();
    let name = birth.name();
    let ready = birth.born_data().and_then(|data| data.first_ready());
    let summary = birth
        .born_data()
        .map(|data| data.summary())
        .unwrap_or_default();
    let mut toast = Card::el(
        BirthComposition::TOAST_PAD_X,
        BirthComposition::TOAST_PAD_Y,
        lex,
    )
    .width(BirthComposition::TOAST_WIDTH)
    .pin_right(BirthComposition::TOAST_INSET)
    .pin_bottom(BirthComposition::TOAST_INSET)
    .align_start()
    .child(
        Element::text(Voice::CAP, lex.mauve, "Roadmap created")
            .claim(Component::Cap)
            .nowrap(),
    )
    .child(air(BirthComposition::TOAST_CAP_BELOW))
    .child(
        Element::row()
            .gap(BirthComposition::SIGIL_GAP)
            .center()
            .child(Element::mark(
                Mark::Sigil(interned(name)),
                BirthComposition::SIGIL_PX,
                Lexicon::ink_bytes(lex.tx),
            ))
            .child(
                Element::text(BirthComposition::NAME, lex.tx, name)
                    .claim(Component::Title)
                    .nowrap(),
            ),
    )
    .child(air(BirthComposition::TOAST_NAME_BELOW))
    .child(
        Element::text(Voice::LEDGER_META, lex.ov0, summary)
            .claim(Component::Cap)
            .width(BirthComposition::TOAST_WIDTH - 2.0 * BirthComposition::TOAST_PAD_X),
    )
    .child(air(BirthComposition::TOAST_LINE_BELOW));
    let claim = ready.map(|id| Action::new(format!("Claim {id}"), BirthComposition::CLAIM));
    toast = toast.child(
        ActionRow::new(claim, vec![Action::new("Later", BirthComposition::LATER)])
            .gap(BirthComposition::TOAST_GAP)
            .el(medium),
    );
    let mut room = Element::surface()
        .grow(1.0)
        .min_height(0.0)
        .fill(lex.base)
        .child(
            Element::surface()
                .pin_left(0.0)
                .pin_top(0.0)
                .pin_right(0.0)
                .pin_bottom(0.0)
                .id(SkyComposition::FIELD),
        );
    if let Some(id) = ready {
        room = room.child(
            Element::text(
                BirthComposition::LABEL,
                lex.mauve,
                format!("{id} \u{b7} Ready"),
            )
            .claim(Component::Label)
            .nowrap()
            .pin_left(size.0 * BirthComposition::READY_LEFT)
            .pin_top(size.1 * BirthComposition::READY_TOP),
        );
    }
    room.child(toast)
}

/// One receipt on the Files written column: the kind's glyph and word
/// in the faint ink at the key width, the value in the mono, the
/// verdict at the far edge in its ink, over a hairline every row but
/// the last.
fn receipt(
    icon: Icon,
    key: &str,
    value: &str,
    verdict: &str,
    verdict_ink: Srgba,
    divided: bool,
    lex: &Lexicon,
) -> Element {
    let row = Element::row()
        .pad_x(BirthComposition::RECEIPT_PAD_X)
        .pad_y(BirthComposition::RECEIPT_PAD_Y)
        .gap(BirthComposition::RECEIPT_GAP)
        .center()
        .child(
            Element::row()
                .min_width(BirthComposition::RECEIPT_KEY)
                .no_shrink()
                .gap(BirthComposition::RECEIPT_KEY_GAP)
                .center()
                .child(Element::mark(
                    Mark::Icon(icon),
                    BirthComposition::RECEIPT_ICON_PX,
                    Lexicon::ink_bytes(lex.ov0),
                ))
                .child(
                    Element::text(Voice::RECEIPT_KEY, lex.ov0, key)
                        .claim(Component::ReceiptRow)
                        .nowrap(),
                ),
        )
        .child(
            Element::text(Voice::VALUE, lex.sb1, value)
                .claim(Component::ReceiptRow)
                .nowrap(),
        )
        .child(Element::row().grow(1.0))
        .child(
            Element::text(Voice::LEDGER_META, verdict_ink, verdict)
                .claim(Component::ReceiptRow)
                .nowrap(),
        );
    if !divided {
        return row;
    }
    let rule = Srgba::new(
        lex.hairc.r,
        lex.hairc.g,
        lex.hairc.b,
        lex.hairc.a * BirthComposition::RULE_SHARE,
    );
    Element::surface().child(row).child(
        Element::surface()
            .height(1.0)
            .fill(Lexicon::composite(lex.base, rule)),
    )
}

/// `row` mid-rise: lifted by the rise's remaining travel and faded by
/// its progress — the volume's `rise` keyframe (`translateY(10px)`,
/// `opacity:0` → none, 1). Landed, the row itself.
fn risen(row: Element, rise: Option<f32>) -> Element {
    match rise {
        Some(t) => {
            let lift = 10.0 * (1.0 - t);
            Element::surface().child(row.pin_top(lift))
        }
        None => row,
    }
}

/// The glyph a survey fact's key wears.
fn survey_icon(kind: magistr_cockpit::cockpit::SurveyKind) -> Icon {
    use magistr_cockpit::cockpit::SurveyKind;
    match kind {
        SurveyKind::Stack => Icon::Stack,
        SurveyKind::Tests => Icon::Gate,
        SurveyKind::Git => Icon::Clone,
        SurveyKind::Agent => Icon::Agent,
    }
}

/// The glyph a scaffold receipt's key wears.
fn scaffold_icon(kind: magistr_cockpit::cockpit::ScaffoldKind) -> Icon {
    use magistr_cockpit::cockpit::ScaffoldKind;
    match kind {
        ScaffoldKind::Config => Icon::Settings,
        ScaffoldKind::Rules => Icon::Report,
        ScaffoldKind::Hooks => Icon::Gate,
        ScaffoldKind::Store => Icon::Stack,
    }
}

/// A rigid run of vertical air.
fn air(px: f32) -> Element {
    Element::surface().height(px.max(0.0))
}

#[cfg(test)]
#[path = "comp_birth.tests.rs"]
mod tests;
