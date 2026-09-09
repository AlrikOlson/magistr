use qs_gpu::color::Srgba;

use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::icon::Icon;
use crate::lexicon::Lexicon;
use crate::mark::Mark;
use crate::voice::Voice;

/// One place in the settings book — the volume's `.sni`: a 12px kind
/// mark beside the group's name, the active one on the accent glow's
/// 13% wash with its mark in the accent. Navigation: it names a place
/// and answers a click.
pub struct SettingsNav;

impl SettingsNav {
    /// `.sni { padding:6px 10px; border-radius:7px; gap:8px }`.
    const PAD_X: f32 = 10.0;
    const PAD_Y: f32 = 6.0;
    const RADIUS: f32 = 7.0;
    const GAP: f32 = 8.0;
    /// `.sni .ic { width:12px; height:12px }`.
    const ICON_PX: u32 = 12;
    /// The active wash — `color-mix(in oklab, var(--glow-m) 13%, transparent)`.
    const WASH: f32 = 0.13;
    /// The active label's weight (`font-weight:550`).
    const ON_VOICE: Voice = Voice {
        weight: 550,
        ..Voice::SLOT
    };

    /// The tag's voice — the inline mono 8.5 (`read-only`).
    const TAG: Voice = Voice {
        px: 8.5,
        ..Voice::CHIP
    };

    /// The item: `icon` marks the group's kind, `label` names it,
    /// `active` lights it, on `ground`, answering on `id`.
    #[must_use]
    pub fn el(
        icon: Icon,
        label: &str,
        active: bool,
        ground: Srgba,
        id: ElementId,
        lex: &Lexicon,
    ) -> Element {
        Self::tagged(icon, label, active, None, ground, id, lex)
    }

    /// The item with a `tag` at its far edge — `read-only` on a team
    /// group, so ownership is visible before the click (view 10).
    #[must_use]
    pub fn tagged(
        icon: Icon,
        label: &str,
        active: bool,
        tag: Option<&str>,
        ground: Srgba,
        id: ElementId,
        lex: &Lexicon,
    ) -> Element {
        let (mark_ink, label_ink) = if active {
            (lex.mauve, lex.tx)
        } else {
            (lex.ov0, lex.sb1)
        };
        let voice = if active { Self::ON_VOICE } else { Voice::SLOT };
        let mut row = Element::row()
            .pad_x(Self::PAD_X)
            .pad_y(Self::PAD_Y)
            .gap(Self::GAP)
            .center()
            .radius(Self::RADIUS)
            .id(id)
            .focusable()
            .focus_ring(lex.mauve);
        if active {
            let wash = Srgba::new(lex.glow_m.r, lex.glow_m.g, lex.glow_m.b, Self::WASH);
            row = row.fill(Lexicon::composite(ground, wash));
        }
        row = row
            .child(Element::mark(
                Mark::Icon(icon),
                Self::ICON_PX,
                Lexicon::ink_bytes(mark_ink),
            ))
            .child(
                Element::text(voice, label_ink, label)
                    .claim(Component::Nav)
                    .nowrap(),
            );
        if let Some(tag) = tag {
            row = row.child(Element::row().grow(1.0)).child(
                Element::text(Self::TAG, lex.ov0, tag)
                    .claim(Component::Nav)
                    .nowrap(),
            );
        }
        row
    }
}

#[cfg(test)]
#[path = "settings_nav.tests.rs"]
mod tests;
