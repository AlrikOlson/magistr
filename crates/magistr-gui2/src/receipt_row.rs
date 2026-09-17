use qs_gpu::color::Srgba;

use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::icon::Icon;
use crate::led::Led;
use crate::lexicon::Lexicon;
use crate::mark::Mark;
use crate::voice::Voice;

/// Evidence, one line at a time — the volume's `.rr`: a fixed-width key naming a
/// **kind** (its icon and tracked uppercase word), the value the work left behind, and
/// either an accent verb or a mode badge at the far edge. receipt row: a commit hash or
/// a gate verdict never floats as prose.
pub struct ReceiptRow;

impl ReceiptRow {
    /// `.rr { gap:10px; padding:6px 2px }`.
    const GAP: f32 = 10.0;
    const PAD_X: f32 = 2.0;
    const PAD_Y: f32 = 6.0;
    /// `.rr .rk { width:58px; flex:none; gap:5px }` with the icon at
    /// the set's own 13px. A floor, not a fixed width: the browser's
    /// `min-width:auto` lets a long key (`branches`) grow the column
    /// past 58 rather than fold its glyph over its word, and the value
    /// follows it.
    const KEY_WIDTH: f32 = 58.0;
    const KEY_GAP: f32 = 5.0;
    const KEY_ICON_PX: u32 = 13;
    /// The row's hairline keeps 60% of the hairline token
    /// (`color-mix(in oklab, var(--hairc) 60%, transparent)`).
    const RULE_SHARE: f32 = 0.6;
    /// `.rr .rv { gap:4px }` — the gate lights' spacing.
    const LED_GAP: f32 = 4.0;

    /// A receipt whose value is a mono run — `commit · a3f9c21 · View`.
    #[must_use]
    pub fn value(
        key: (Icon, &str),
        value: &str,
        action: Option<&str>,
        divided: bool,
        ground: Srgba,
        lex: &Lexicon,
    ) -> Element {
        let value = Element::text(Voice::VALUE, lex.sb1, value)
            .claim(Component::ReceiptRow)
            .nowrap();
        Self::row(
            Some(key),
            value,
            action.map(|a| (a, None)),
            None,
            divided,
            ground,
            lex,
        )
    }

    /// A receipt whose accent verb answers a click — `source ·
    /// ~/.config/magistr/config.toml · Open file` (view 10): the verb
    /// carries `id`.
    #[must_use]
    pub fn linked(
        key: (Icon, &str),
        value: &str,
        action: &str,
        id: ElementId,
        divided: bool,
        ground: Srgba,
        lex: &Lexicon,
    ) -> Element {
        let value = Element::text(Voice::VALUE, lex.sb1, value)
            .claim(Component::ReceiptRow)
            .nowrap();
        Self::row(
            Some(key),
            value,
            Some((action, Some(id))),
            None,
            divided,
            ground,
            lex,
        )
    }

    /// A receipt whose value is the gate lights — `gates · ●●○ · Log`.
    #[must_use]
    pub fn leds(
        key: (Icon, &str),
        lit: &[bool],
        action: Option<&str>,
        divided: bool,
        ground: Srgba,
        lex: &Lexicon,
    ) -> Element {
        let mut lights = Element::row().gap(Self::LED_GAP).center();
        for on in lit {
            lights = lights.child(Led::el(*on, lex));
        }
        Self::row(
            Some(key),
            lights,
            action.map(|a| (a, None)),
            None,
            divided,
            ground,
            lex,
        )
    }

    /// A receipt whose value is the gate lights *and* their count, with
    /// an accent verb that answers a click — `gates · ●●● 3 passed ·
    /// Log` (view 09): the lights, then `note` in the small mono the
    /// volume sets beside them (`margin-left:4px`), the verb carrying
    /// `id`.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn lit_linked(
        key: (Icon, &str),
        lit: &[bool],
        note: &str,
        action: &str,
        id: ElementId,
        divided: bool,
        ground: Srgba,
        lex: &Lexicon,
    ) -> Element {
        let mut lights = Element::row().gap(Self::LED_GAP).center();
        for on in lit {
            lights = lights.child(Led::el(*on, lex));
        }
        let value = Element::row()
            .gap(Self::LED_GAP)
            .center()
            .child(lights)
            .child(
                Element::text(Voice::CHIP, lex.sb1, note)
                    .claim(Component::ReceiptRow)
                    .nowrap(),
            );
        Self::row(
            Some(key),
            value,
            Some((action, Some(id))),
            None,
            divided,
            ground,
            lex,
        )
    }

    /// A rule and its standing answer — `git push · OFF`: no key
    /// column, the rule's own words as the value, the mode badge at
    /// the far edge.
    #[must_use]
    pub fn rule(
        rule: &str,
        badge: Element,
        divided: bool,
        ground: Srgba,
        lex: &Lexicon,
    ) -> Element {
        let value = Element::text(Voice::VALUE, lex.sb1, rule)
            .claim(Component::ReceiptRow)
            .nowrap();
        Self::row(None, value, None, Some(badge), divided, ground, lex)
    }

    #[allow(clippy::too_many_arguments)]
    fn row(
        key: Option<(Icon, &str)>,
        value: Element,
        action: Option<(&str, Option<ElementId>)>,
        badge: Option<Element>,
        divided: bool,
        ground: Srgba,
        lex: &Lexicon,
    ) -> Element {
        let mut row = Element::row()
            .pad_x(Self::PAD_X)
            .pad_y(Self::PAD_Y)
            .gap(Self::GAP)
            .center();
        if let Some((icon, word)) = key {
            row = row.child(
                Element::row()
                    .min_width(Self::KEY_WIDTH)
                    .no_shrink()
                    .gap(Self::KEY_GAP)
                    .center()
                    .child(Element::mark(
                        Mark::Icon(icon),
                        Self::KEY_ICON_PX,
                        Lexicon::ink_bytes(lex.ov0),
                    ))
                    .child(
                        Element::text(Voice::RECEIPT_KEY, lex.ov0, word)
                            .claim(Component::ReceiptRow)
                            .nowrap(),
                    ),
            );
        }
        row = row.child(value).child(Element::row().grow(1.0));
        if let Some((action, id)) = action {
            let mut verb = Element::text(Voice::LEDGER_META, lex.mauve, action)
                .claim(Component::ReceiptRow)
                .nowrap();
            if let Some(id) = id {
                verb = verb.id(id).focusable().focus_ring(lex.mauve);
            }
            row = row.child(verb);
        }
        if let Some(badge) = badge {
            row = row.child(badge);
        }
        if !divided {
            return row;
        }
        let rule_tint = Srgba::new(
            lex.hairc.r,
            lex.hairc.g,
            lex.hairc.b,
            lex.hairc.a * Self::RULE_SHARE,
        );
        Element::surface().child(row).child(
            Element::surface()
                .height(1.0)
                .fill(Lexicon::composite(ground, rule_tint)),
        )
    }
}

#[cfg(test)]
#[path = "receipt_row.tests.rs"]
mod tests;
