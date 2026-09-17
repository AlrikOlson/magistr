use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::lexicon::Lexicon;
use crate::voice::Voice;

/// The meter with its limit tick — the volume's `.meter`: a cap and a mono value over a
/// 4px track, the fill in the success hue, the breaker tick in the warning's standing
/// 2px proud of the track both ways. A quantity you *read*: like the data pill the
/// meter itself carries no id, no focus, no pointer answer. The one control near it is
/// the rail's `Raise` on the foot line, which the caller asks for by id and which is
/// the only thing in the tree that answers a pointer.
pub struct BudgetMeter;

impl BudgetMeter {
    /// The foot line's `Raise` on the rail — the id the root resolves
    /// to the cockpit's budget widget.
    pub const RAISE: ElementId = ElementId(510);
    /// The track's geometry (`.mb { height:4px; border-radius:2px }`,
    /// `.mb b { width:2px; top:-2px; bottom:-2px }`) and the label
    /// row's air (`.ml { margin-bottom:5px }`, `.mf { margin-top:4px }`).
    const TRACK_H: f32 = Voice::METER_TRACK_HEIGHT;
    const TRACK_RADIUS: f32 = 2.0;
    const TICK_W: f32 = Voice::METER_TICK_WIDTH;
    const TICK_OVER: f32 = 2.0;
    const LABEL_GAP: f32 = 5.0;
    /// `.ml { align-items:baseline }`: the browser sets the cap and the
    /// value on one baseline. Bottom-aligning their boxes instead puts
    /// the 9.5px cap's baseline 1px under the 10.5px value's — the
    /// difference of the two lines' descents-plus-half-leading — so
    /// the cap's box carries that pixel as bottom padding.
    const LABEL_BASELINE_LIFT: f32 = 1.0;
    const NOTE_GAP: f32 = 4.0;
    /// `.mf` is a block of the window's own line — `.app7`'s 12.5px
    /// sans at its 1.5 leading — and the 9.5px cap inside it sits on
    /// *that* line's baseline, not its own: 3px lower than the cap's
    /// line box alone would put it, the two ascents' difference
    /// measured on view 05's capture. The margin is the volume's 4;
    /// this is the strut's.
    const NOTE_BASELINE_DROP: f32 = 3.0;

    /// The meter: `label` and `value` over the track, `fill` and
    /// `tick` as shares of `width` (the CSS speaks percentages), and
    /// the foot line under it — the optional stop `note` at its start
    /// and, when the caller hands an id, `Raise` at its far edge.
    #[must_use]
    // The volume's meter has exactly these parts; a builder for one
    // component would be a second vocabulary for them.
    #[allow(clippy::too_many_arguments)]
    pub fn el(
        label: &str,
        value: &str,
        fill: f32,
        tick: Option<f32>,
        note: Option<&str>,
        raise: Option<ElementId>,
        width: f32,
        lex: &Lexicon,
    ) -> Element {
        let track = Self::track(fill, tick, width, lex);
        // The label row is exactly the value voice's own line box —
        // the browser's 15.75, not that rounded to 16 — so the track
        // lands on the same fractional row the capture's did.
        let label_row_h = Voice::METER_VALUE.px * Voice::METER_VALUE.leading();
        let mut meter = Element::surface()
            .child(
                Element::row()
                    .height(label_row_h)
                    .align_end()
                    .space_between()
                    .child(
                        Element::surface()
                            .pad_bottom(Self::LABEL_BASELINE_LIFT)
                            .child(
                                Element::text(Voice::CAP, lex.ov0, label)
                                    .claim(Component::Meter)
                                    .nowrap(),
                            ),
                    )
                    .child(
                        Element::text(Voice::METER_VALUE, lex.sb1, value)
                            .claim(Component::Meter)
                            .nowrap(),
                    ),
            )
            .child(Element::surface().height(Self::LABEL_GAP))
            .child(track);
        if note.is_some() || raise.is_some() {
            // `.mf { display:flex; align-items:baseline }`: the note at
            // the start, the spacer, the verb at the far edge — both
            // runs 9.5px mono, so their boxes share a bottom.
            let mut foot = Element::row().align_end();
            if let Some(note) = note {
                foot = foot.child(
                    Element::text(Voice::METER_NOTE, lex.ov0, note)
                        .claim(Component::Meter)
                        .nowrap(),
                );
            }
            foot = foot.child(Element::row().grow(1.0));
            if let Some(id) = raise {
                foot = foot.child(
                    Element::text(Voice::RAIL_VERB, lex.mauve, "Raise")
                        .claim(Component::Meter)
                        .nowrap()
                        .id(id)
                        .focusable()
                        .focus_ring(lex.mauve),
                );
            }
            meter = meter
                .child(Element::surface().height(Self::NOTE_GAP + Self::NOTE_BASELINE_DROP))
                .child(foot);
        }
        meter
    }

    /// The track alone — the volume's bare `.mb`, which the Galaxy's
    /// foot lays beside its own cap and value: `fill` and `tick` as
    /// shares of `width`.
    #[must_use]
    pub fn track(fill: f32, tick: Option<f32>, width: f32, lex: &Lexicon) -> Element {
        let mut track = Element::row()
            .width(width)
            .height(Self::TRACK_H)
            .radius(Self::TRACK_RADIUS)
            .fill(lex.s0)
            .child(
                Element::surface()
                    .width(width * fill.clamp(0.0, 1.0))
                    .radius(Self::TRACK_RADIUS)
                    .fill(lex.green)
                    .pin_left(0.0)
                    .pin_top(0.0)
                    .pin_bottom(0.0),
            );
        if let Some(tick) = tick {
            track = track.child(
                Element::surface()
                    .width(Self::TICK_W)
                    .radius(1.0)
                    .fill(lex.yellow)
                    .pin_left(width * tick.clamp(0.0, 1.0))
                    .pin_top(-Self::TICK_OVER)
                    .pin_bottom(-Self::TICK_OVER),
            );
        }
        track
    }
}

#[cfg(test)]
#[path = "budget_meter.tests.rs"]
mod tests;
