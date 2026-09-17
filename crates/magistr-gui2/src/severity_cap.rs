use magistr_cockpit::theme::Severity;

use crate::component::Component;
use crate::element::Element;
use crate::icon::Icon;
use crate::lexicon::Lexicon;
use crate::mark::Mark;
use crate::severity::SeverityInk;
use crate::voice::Voice;

/// A card's coloured cap — the volume's `.cap7` worn with a severity: the band's
/// mark-sized icon and its tracked uppercase word, both in the band's full-strength
/// hue. The *cap* half of tinted-ground-plus-cap treatment, and law in miniature: the
/// icon names a **kind** (report, alert, check, signal), never a control.
pub struct SeverityCap;

impl SeverityCap {
    /// `.cap7 svg.ic { width:11px; height:11px }`.
    const ICON_PX: u32 = 11;

    /// The cap: `copy` in `severity`'s hue, led by its kind `icon`
    /// when the card wears one, on one unbreakable row. The space
    /// after the icon is the volume's own — a literal space in the
    /// run, not a flex gap — so the cap's tracking spreads it the way
    /// the browser does.
    #[must_use]
    pub fn el(severity: Severity, icon: Option<Icon>, copy: &str, lex: &Lexicon) -> Element {
        Self::tinted(SeverityInk::of(severity, lex).cap, icon, copy)
    }

    /// The same cap in a hue that is not a severity band — the accent
    /// the think-pin's `think · 388 · pinned` wears. The component is
    /// the cap; the band is one way to ink it.
    ///
    /// The word wraps: `.cap7` declares no `nowrap` (the chip does),
    /// and a squeezed cap in the volume genuinely breaks onto a second
    /// line rather than clipping.
    #[must_use]
    pub fn tinted(hue: qs_gpu::color::Srgba, icon: Option<Icon>, copy: &str) -> Element {
        let row = Element::row().center();
        match icon {
            Some(icon) => row
                .child(Element::mark(
                    Mark::Icon(icon),
                    Self::ICON_PX,
                    Lexicon::ink_bytes(hue),
                ))
                .child(Element::text(Voice::CAP, hue, format!(" {copy}")).claim(Component::Cap)),
            None => row.child(Element::text(Voice::CAP, hue, copy).claim(Component::Cap)),
        }
    }
}

#[cfg(test)]
#[path = "severity_cap.tests.rs"]
mod tests;
