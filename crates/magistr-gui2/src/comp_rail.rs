use magistr_cockpit::cockpit::{CharterOffer, LoopStatus, Model, RuleMode, Spine, Widget};
use qs_gpu::color::Srgba;

use crate::budget_meter::BudgetMeter;
use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::icon::Icon;
use crate::ledger_verbs::LedgerVerbs;
use crate::lexicon::Lexicon;
use crate::mark::Mark;
use crate::medium::Medium;
use crate::mode_badge::ModeBadge;
use crate::receipt_row::ReceiptRow;
use crate::spine_ledger::SpineLedger;
use crate::voice::Voice;

/// The supervision rail — three questions, composed from the model on the volume's own
/// geometry: the checkpoint spine under its cap, the charter's rules with their mode
/// badges under `Charter · <mode>`, and the bound budget's meter. Every row is a
/// verified component; an empty section shows its empty state rather than furniture;
/// the composition itself draws nothing but the seam.
///
/// The width is the volume's 248 — the hairline included, the way the capture's
/// `.hair-l` box counts it — moved by whole cells when the user drags the divider
/// ([`Rail::dragged_cols`](magistr_cockpit::cockpit::Rail)), so the default rail is the
/// design's to the pixel, the divider drag still reshapes this composition through the
/// one shared `update`, and no width state lives here.
pub struct RailComposition;

impl RailComposition {
    /// Ledger rows the spine shows under its head — the newest states;
    /// the full chain stays reachable through the palette. The cell
    /// rail's own count (`NATIVE_SPINE_ROWS`).
    const SPINE_ROWS: usize = 2;
    /// A cap's glyph (`.cap7 svg.ic { width:11px }`) and the one space
    /// the volume sets between it and the word — a 9.5px mono space
    /// plus the cap's `.12em` tracking after it, 6.8px, which the
    /// capture's word starts on the far side of.
    const CAP_ICON_PX: u32 = 11;
    const CAP_GAP: f32 = 7.0;
    /// The air under the agent block's cap (`margin-bottom:6px`).
    const AGENT_CAP_GAP: f32 = 6.0;
    /// A charter row's id, by its index in the charter — a hit target that raises under
    /// the pointer and is named read-only until the delegation dial lands. The space
    /// sits past the spine's three.
    pub const CHARTER: u64 = 1 << 36;
    /// The end of the charter space.
    pub const CHARTER_END: u64 = 1 << 37;
    /// A charter offer's `Delegate`, by its index in the offers — the verb that accepts
    /// the widening the reader's record supports. Past the charter's space.
    pub const OFFER: u64 = 1 << 37;
    /// The end of the offer space.
    pub const OFFER_END: u64 = 1 << 38;
    /// The gap between an offer's subject and its verb, and between
    /// its first line and its record.
    const OFFER_GAP: f32 = 4.0;
    /// A raised row's ground — the spine row's own `color-mix(in
    /// oklab, var(--glow-m) 9%, transparent)` at `border-radius:6px`.
    const RAISE_SHARE: f32 = 0.09;
    const RAISE_RADIUS: f32 = 6.0;
    /// What a charter row says of itself: the rule stands, and nothing on the row
    /// changes it — a rule is written by answering an ask or accepting an offer, never
    /// by editing the row.
    const CHARTER_READ_ONLY: &'static str = "read-only";

    /// The rail for `model`, in `medium`'s lexicon. `cell_w` is the
    /// surface's own cell advance in pixels — the renderer knows its
    /// [`CellMetrics`](crate::CellMetrics); the drag is measured in
    /// columns on the model, so a dragged column moves the rail by one
    /// advance on both surfaces and the two cannot disagree about it.
    #[must_use]
    pub fn el(model: &Model, medium: Medium, cell_w: f32) -> Element {
        let lex = Lexicon::of(medium);
        let rail = model.rail();
        // The volume's 248, moved by the dragged columns: the cell
        // rail's 28 columns × 9px put the seam at 848 where the
        // capture's is at 852, which is 4px off every left-aligned run.
        // A drag is a handful of columns; f32 holds the count exactly.
        #[allow(clippy::cast_precision_loss)]
        let dragged = rail.dragged_cols(model.size().cols) as f32;
        let width = Voice::RAIL_WIDTH + dragged * cell_w;
        let column_w = width - 1.0;
        let inner = column_w - 2.0 * Voice::RAIL_PAD_SIDE;
        let mut column = Element::surface()
            .width(column_w)
            .pad_x(Voice::RAIL_PAD_SIDE)
            .pad_y(Voice::RAIL_PAD_TOP)
            .align_start();

        // Checkpoints — the spine of states the session can be taken back to, the head
        // first wearing the loop's verbs, and the newest under it, each a control:
        // Preview always, Restore · Fork on the row the pointer rests on or the one
        // being previewed.
        let spine = rail.spine();
        column = column.child(cap(Some(Icon::Checkpoint), "Checkpoints", &lex));
        column = column.child(if spine.is_empty() {
            empty("No checkpoints yet", &lex)
        } else {
            let paused = model.loop_status().is_some_and(LoopStatus::is_paused);
            let verbs = LedgerVerbs::el(paused, medium, lex.base, &lex);
            let revealed = revealed_row(model, spine);
            Element::surface()
                .width(inner)
                .child(air(Voice::RAIL_CAP_GAP))
                .child(SpineLedger::el(
                    spine,
                    Self::SPINE_ROWS,
                    Some(verbs),
                    revealed,
                    &lex,
                ))
        });

        // Charter — one receipt row per rule, wearing its mode badge,
        // under the cap that names how far the loop is trusted. Each
        // row is a hit target that raises under the pointer and says it
        // is read-only: a rule is written by answering an ask or
        // accepting an offer below, and a row that looked editable
        // would be lying.
        let charter = rail.charter();
        let raise = raise(&lex);
        let charter_cap = match rail.mode() {
            Some(mode) => format!("Charter \u{b7} {mode}"),
            None => "Charter".to_owned(),
        };
        column = column.child(air(Voice::RAIL_SECTION_GAP)).child(cap(
            Some(Icon::Agent),
            &charter_cap,
            &lex,
        ));
        column = column.child(if charter.is_empty() {
            empty("No rules bound", &lex)
        } else {
            let mut rules = Element::surface()
                .width(inner)
                .child(air(Voice::RAIL_SECTION_CAP_GAP));
            for (index, rule) in charter.iter().enumerate() {
                let badge = match rule.mode() {
                    RuleMode::Ask => ModeBadge::ask(lex.base, &lex),
                    RuleMode::Auto => ModeBadge::auto(lex.base, &lex),
                    RuleMode::Off => ModeBadge::off(lex.base, &lex),
                };
                let id = ElementId(
                    Self::CHARTER.saturating_add(u64::try_from(index).unwrap_or(u64::MAX)),
                );
                rules = rules.child(
                    ReceiptRow::rule(
                        rule.subject(),
                        badge,
                        index + 1 < charter.len(),
                        lex.base,
                        &lex,
                    )
                    .id(id)
                    .focusable()
                    .focus_ring(lex.mauve)
                    .radius(Self::RAISE_RADIUS)
                    .hover_fill(raise)
                    .label(format!(
                        "{} \u{b7} {} \u{b7} {}",
                        rule.subject(),
                        rule.mode().label(),
                        Self::CHARTER_READ_ONLY
                    )),
                );
            }
            rules
        });

        // The charter's offers: a scope the reader has allowed every ask under, often
        // enough that the charter proposes to stop asking — the subject, its record in
        // the meta voice, and `Delegate` at the edge, the panel's one verb.
        let offers = rail.offers();
        if !offers.is_empty() {
            let mut rows = Element::surface()
                .width(inner)
                .child(air(Voice::RAIL_SECTION_CAP_GAP));
            for (index, offer) in offers.iter().enumerate() {
                let id =
                    ElementId(Self::OFFER.saturating_add(u64::try_from(index).unwrap_or(u64::MAX)));
                rows = rows.child(offer_row(offer, id, &lex));
            }
            column = column.child(rows);
        }

        // Budget — the bound meter with `Raise` on its foot line, or the section's
        // honest absence.
        column = column.child(air(Voice::RAIL_SECTION_GAP));
        column = column.child(match rail.budget() {
            Some(budget) => Element::surface().width(inner).child(BudgetMeter::el(
                "Budget",
                &budget.readout(),
                f32::from(budget.fill_per_mille()) / 1000.0,
                budget.tick_per_mille().map(|t| f32::from(t) / 1000.0),
                budget.breaker().as_deref(),
                Some(BudgetMeter::RAISE),
                inner,
                &lex,
            )),
            None => Element::surface()
                .align_start()
                .child(cap(None, "Budget", &lex))
                .child(empty("No budget bound", &lex)),
        });

        // The agent — the model under its mode word and the ledger line, once the
        // handshake named one. Nothing is drawn before that: there is no agent to name
        // during the handshake, and a row saying so for two seconds would be furniture.
        if let Some(agent) = rail.agent() {
            column = column.child(air(Voice::RAIL_SECTION_GAP)).child(
                Element::surface()
                    .width(inner)
                    .align_start()
                    .child(cap(Some(Icon::Session), "Agent", &lex))
                    .child(air(Self::AGENT_CAP_GAP))
                    .child(
                        Element::text(Voice::LEDGER_NAME, lex.sb1, agent.identity())
                            .claim(Component::Value)
                            .nowrap(),
                    )
                    .child(
                        Element::text(Voice::LEDGER_META, lex.sb0, agent.ledger())
                            .claim(Component::Value)
                            .nowrap(),
                    ),
            );
        }

        // The volume's `.hair-l`: the seam is the rail's own left edge,
        // inside its 248.
        Element::row()
            .width(width)
            .no_shrink()
            .child(
                Element::surface()
                    .width(1.0)
                    .stretch()
                    .fill(lex.grounded(lex.hairc)),
            )
            .child(column)
    }
}

/// The spine index of the checkpoint row whose Restore · Fork are
/// revealed: the row the pointer rests on — or one of its own verbs,
/// so walking the pointer from the row onto Restore keeps Restore
/// there (WCAG 1.4.13's hoverable) — else the row being previewed,
/// which is how a keyboard reaches the pair: Tab to the row, ↵ to
/// preview it, and the verbs join the ring. The pointer's reading is
/// [`Model::hovered`], the hit registry's target on the last move, so
/// the composition and the input layer cannot disagree about it.
fn revealed_row(model: &Model, spine: &Spine) -> Option<usize> {
    let hovered = model.hovered().and_then(|id| {
        Widget::spine_row_index(id)
            .or_else(|| Widget::spine_restore_index(id))
            .or_else(|| Widget::spine_fork_index(id))
    });
    hovered
        .or_else(|| spine.selected())
        .filter(|index| *index < spine.len())
}

/// One offer's entry, two lines the way the spine's head is: the scope
/// in the receipt voice with `Delegate` at the far edge in the rail's
/// accent voice — a focusable control carrying `id`, the way a
/// checkpoint row's Restore is — and its record, `allowed 14 of 14`,
/// in the meta voice under it. The subject gives way before the verb.
fn offer_row(offer: &CharterOffer, id: ElementId, lex: &Lexicon) -> Element {
    Element::surface()
        .align_start()
        .stretch()
        .gap(RailComposition::OFFER_GAP)
        .child(
            Element::row()
                .gap(RailComposition::OFFER_GAP)
                .align_end()
                .stretch()
                .child(
                    Element::text(Voice::VALUE, lex.sb1, offer.subject())
                        .claim(Component::ReceiptRow)
                        .nowrap()
                        .min_width(0.0),
                )
                .child(Element::row().grow(1.0))
                .child(
                    Element::text(Voice::RAIL_VERB, lex.mauve, "Delegate")
                        .claim(Component::Ledger)
                        .nowrap()
                        .no_shrink()
                        .id(id)
                        .focusable()
                        .focus_ring(lex.mauve),
                ),
        )
        .child(
            Element::text(Voice::LEDGER_META, lex.ov0, offer.tally())
                .claim(Component::ReceiptRow)
                .nowrap(),
        )
}

/// A raised row's ground on the rail — the glow at its 9% over the
/// base, the same ground the spine's rows raise onto.
fn raise(lex: &Lexicon) -> Srgba {
    let glow = lex.glow_m;
    Lexicon::composite(
        lex.base,
        Srgba::new(glow.r, glow.g, glow.b, RailComposition::RAISE_SHARE),
    )
}

/// A section cap in the volume's own voice, with its glyph when it
/// wears one.
fn cap(icon: Option<Icon>, text: &str, lex: &Lexicon) -> Element {
    let mut row = Element::row().gap(RailComposition::CAP_GAP).center();
    if let Some(icon) = icon {
        row = row.child(Element::mark(
            Mark::Icon(icon),
            RailComposition::CAP_ICON_PX,
            Lexicon::ink_bytes(lex.ov0),
        ));
    }
    row.child(
        Element::text(Voice::CAP, lex.ov0, text)
            .claim(Component::Cap)
            .nowrap(),
    )
}

/// Vertical air of `px` between two blocks — the volume's margins,
/// spelled where they fall rather than as one uniform gap.
fn air(px: f32) -> Element {
    Element::surface().height(px)
}

/// A muted one-row empty state under a section's cap — honest about
/// there being nothing, never furniture standing in for data.
fn empty(text: &str, lex: &Lexicon) -> Element {
    Element::surface()
        .align_start()
        .child(air(Voice::RAIL_SECTION_CAP_GAP))
        .child(
            Element::text(Voice::SLOT, lex.sb0, text)
                .claim(Component::Slot)
                .nowrap(),
        )
}

#[cfg(test)]
#[path = "comp_rail.tests.rs"]
mod tests;
