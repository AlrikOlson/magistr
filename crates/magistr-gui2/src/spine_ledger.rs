use magistr_cockpit::cockpit::{Checkpoint, Spine};
use magistr_cockpit::theme::Icons;
use qs_gpu::color::Srgba;

use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::ledger_verbs::LedgerVerbs;
use crate::lexicon::Lexicon;
use crate::voice::Voice;

/// The checkpoint spine — view 05's ledger of the states a session can be taken back
/// to, as the volume draws it: a 2px line down the left with a 9px node on every entry,
/// the head entry first (`head now` over `N receipts after cp-N`, its node in the
/// accent because it is the live state), then the newest checkpoints under it, each
/// `cp-N · label` wearing the gate verdicts its iteration ran in the success hue.
/// ledger head and checkpoint rows.
///
/// The head's verbs are the caller's
/// ([`LedgerVerbs`](crate::ledger_verbs::LedgerVerbs)), seated under the receipts line
/// where the capture draws them. A checkpoint row is itself a control: it wears the
/// spine row's id and its far edge carries `Preview` always; the *revealed* row — the
/// one under the pointer or the one being previewed — carries `· Restore · Fork` too,
/// each verb its own id, on the glow ground the eighth edition's II.k raises it on.
/// Which row is revealed is the caller's reading of the model; this block only draws
/// it. The receipts count is the spine's one counter, so the head line and a card's
/// `receipt #N` cannot disagree.
pub struct SpineLedger;

impl SpineLedger {
    /// A checkpoint row's id, by the row's index on the spine — the row
    /// is its own Preview. The spaces sit past every composition's,
    /// under the feed's heads.
    pub const ROWS: u64 = 1 << 33;
    /// A row's revealed `Restore`, by the same index.
    pub const RESTORE: u64 = 1 << 34;
    /// A row's revealed `Fork`, by the same index.
    pub const FORK: u64 = 1 << 35;
    /// The end of the fork space.
    pub const FORK_END: u64 = 1 << 36;
    /// `border-left:2px solid var(--s0)`.
    const LINE_W: f32 = 2.0;
    /// A node: 9px round, `left:-20px; top:3px` off its entry — which
    /// puts it astride the line.
    const NODE: f32 = 9.0;
    const NODE_LEFT: f32 = -20.0;
    const NODE_TOP: f32 = 3.0;
    /// The one space the volume sets between `head` and its `now` —
    /// the 12px sans's space, 3px on the capture.
    const HEAD_GAP: f32 = 3.0;
    /// The space between a row's label and its marks, in the same sans.
    const MARK_GAP: f32 = 3.0;
    /// `gap:6px` between a row's label and its verb.
    const VERB_GAP: f32 = 6.0;
    /// The revealed row's ground — `color-mix(in oklab, var(--glow-m)
    /// 9%, transparent)` over the rail — and its `border-radius:6px`.
    const RAISE_SHARE: f32 = 0.09;
    const RAISE_RADIUS: f32 = 6.0;

    /// The block for `spine`: the head entry — with `verbs`, the loop's,
    /// seated under its receipts line when the caller drew them — over
    /// its newest `rows` checkpoints, newest first, the row at spine
    /// index `revealed` wearing its Restore · Fork. An empty spine has
    /// no head to stand on and answers an empty surface — the caller's
    /// empty state says so in words.
    #[must_use]
    pub fn el(
        spine: &Spine,
        rows: usize,
        verbs: Option<Element>,
        revealed: Option<usize>,
        lex: &Lexicon,
    ) -> Element {
        let Some(head) = spine.head() else {
            return Element::surface();
        };
        let receipts = spine.receipts_after_head().unwrap_or(0);
        let after = match receipts {
            1 => format!("1 receipt after {}", head.id()),
            n => format!("{n} receipts after {}", head.id()),
        };
        let mut head_entry = Element::surface()
            .align_start()
            .child(node(lex.mauve))
            .child(
                Element::row()
                    .gap(Self::HEAD_GAP)
                    .align_end()
                    .child(run(Voice::LEDGER_NAME, lex.tx, "head"))
                    .child(run(Voice::LEDGER_META, lex.ov0, "now")),
            )
            .child(run(Voice::LEDGER_META, lex.ov0, after));
        if let Some(verbs) = verbs {
            // The row spans the entry and wraps inside it (the volume's
            // `flex-wrap:wrap`): the entry fills the column, the column
            // fills what the line and the indent leave, and neither of
            // those two ever shrinks — a row wider than the column must
            // break, never push the spine off its line.
            head_entry = head_entry
                .stretch()
                .child(Element::surface().height(LedgerVerbs::TOP))
                .child(verbs.stretch());
        }
        let mut entries = Element::surface()
            .gap(Voice::RAIL_LEDGER_PITCH)
            .align_start()
            .grow(1.0)
            .min_width(0.0)
            .child(head_entry);
        for (index, cp) in spine.entries().iter().enumerate().rev().take(rows) {
            entries = entries.child(
                Element::surface()
                    .align_start()
                    .stretch()
                    .child(node(lex.s1))
                    .child(row(cp, index, revealed == Some(index), lex)),
            );
        }
        Element::row()
            .align_start()
            .child(
                Element::surface()
                    .width(Self::LINE_W)
                    .no_shrink()
                    .stretch()
                    .fill(lex.s0),
            )
            .child(
                Element::surface()
                    .width(Voice::RAIL_LEDGER_INDENT)
                    .no_shrink(),
            )
            .child(entries)
    }

    /// The revealed row's ground on `base`.
    fn raise(lex: &Lexicon) -> Srgba {
        let glow = lex.glow_m;
        Lexicon::composite(
            lex.base,
            Srgba::new(glow.r, glow.g, glow.b, Self::RAISE_SHARE),
        )
    }
}

/// One checkpoint's row — `cp-4 · before golden update`, after a
/// boundary's label its gate verdicts in the success hue, in the icon
/// set's own glyphs, and at the far edge its verbs: `Preview` always,
/// `· Restore · Fork` while revealed. The row is the spine row's hit
/// target (a click previews), takes focus, and raises onto the glow
/// ground under the pointer.
fn row(cp: &Checkpoint, index: usize, revealed: bool, lex: &Lexicon) -> Element {
    let label = format!("{} \u{b7} {}", cp.id(), cp.label());
    let raise = SpineLedger::raise(lex);
    let index = u64::try_from(index).unwrap_or(u64::MAX);
    let mut row = Element::row()
        .gap(SpineLedger::VERB_GAP)
        .align_end()
        .stretch()
        .radius(SpineLedger::RAISE_RADIUS)
        .hover_fill(raise)
        .id(ElementId(SpineLedger::ROWS.saturating_add(index)))
        .focusable()
        .focus_ring(lex.mauve);
    if revealed {
        row = row.fill(raise);
    }
    // The label gives way (`min-width:0; overflow:hidden`): squeezed
    // by the revealed verbs it clips at its box — the substrate's cut,
    // without the volume's ellipsis glyph — while the marks and the
    // verbs keep their width, so a verdict and a verb are never the
    // thing that goes.
    row = match cp.gate_marks(&Icons::UNICODE) {
        Some(marks) => row.child(
            Element::row()
                .gap(SpineLedger::MARK_GAP)
                .align_end()
                .min_width(0.0)
                .child(run(Voice::SLOT, lex.sb1, format!("{label} \u{b7}")).min_width(0.0))
                .child(run(Voice::SLOT, lex.green, marks).no_shrink()),
        ),
        None => row.child(run(Voice::SLOT, lex.sb1, label).min_width(0.0)),
    };
    row = row
        .child(Element::row().grow(1.0))
        .child(verb("Preview", None, lex));
    if revealed {
        row = row
            .child(verb("\u{b7}", None, lex))
            .child(verb(
                "Restore",
                Some(ElementId(SpineLedger::RESTORE.saturating_add(index))),
                lex,
            ))
            .child(verb("\u{b7}", None, lex))
            .child(verb(
                "Fork",
                Some(ElementId(SpineLedger::FORK.saturating_add(index))),
                lex,
            ));
    }
    row
}

/// A verb at the row's edge in the rail's accent voice — a control
/// when it carries `id`, the row's own word otherwise.
fn verb(text: &str, id: Option<ElementId>, lex: &Lexicon) -> Element {
    let mut verb = Element::text(Voice::RAIL_VERB, lex.mauve, text)
        .claim(Component::Ledger)
        .nowrap()
        .no_shrink();
    if let Some(id) = id {
        verb = verb.id(id).focusable().focus_ring(lex.mauve);
    }
    verb
}

/// A node on the line, pinned off its entry.
fn node(ink: Srgba) -> Element {
    Element::surface()
        .width(SpineLedger::NODE)
        .height(SpineLedger::NODE)
        .radius(SpineLedger::NODE / 2.0)
        .fill(ink)
        .pin_left(SpineLedger::NODE_LEFT)
        .pin_top(SpineLedger::NODE_TOP)
}

/// A run of the ledger's, in `voice` and `ink`.
fn run(voice: Voice, ink: Srgba, text: impl Into<String>) -> Element {
    Element::text(voice, ink, text)
        .claim(Component::Ledger)
        .nowrap()
}

#[cfg(test)]
#[path = "spine_ledger.tests.rs"]
mod tests;
