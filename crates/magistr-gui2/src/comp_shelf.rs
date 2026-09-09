use magistr_cockpit::cockpit::{Shelf, StillKind};
use qs_gpu::color::Srgba;

use crate::action::Action;
use crate::action_row::ActionRow;
use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::icon::Icon;
use crate::lexicon::Lexicon;
use crate::medium::Medium;
use crate::receipt_row::ReceiptRow;
use crate::voice::Voice;

/// The deliverables shelf on a completed chunk's card, including preview images,
/// recording, commit, verification receipts, and export controls.
///
/// Missing artifacts retain their labeled rows and explain their absence. The
/// composition reads [`Shelf`](magistr_cockpit::cockpit::Shelf) without inventing
/// results.
pub struct ShelfComposition;

impl ShelfComposition {
    /// The \[Export\] verb.
    pub const EXPORT: ElementId = ElementId(212);

    /// A thumbnail's height; its width is the picture's own aspect.
    const THUMB_HEIGHT: f32 = 56.0;
    /// Between thumbnails, and between the wrapped rows of them.
    const THUMB_GAP: f32 = 6.0;
    /// Pictures the gallery shows before it counts the rest — the
    /// card is a shelf, not a contact sheet; the bundle carries all.
    const THUMBS: usize = 8;
    /// The section's own rhythm.
    const GAP: f32 = 8.0;

    /// The shelf section for `shelf`, `inner` pixels wide on a card
    /// whose ground is `ground` (the overlay glass on the Sky).
    #[must_use]
    pub fn el(shelf: &Shelf, inner: f32, ground: Srgba, medium: Medium) -> Element {
        let lex = Lexicon::of(medium);
        let stills = shelf.count(StillKind::Still);
        let films = shelf.count(StillKind::Filmstrip);
        let mut cap = Element::row().gap(Self::GAP).center().child(
            Element::text(Voice::CAP, lex.ov0, "deliverables")
                .claim(Component::Cap)
                .nowrap(),
        );
        cap = cap.child(
            Element::text(
                Voice::CAP,
                lex.sb0,
                format!(
                    "{} {} \u{b7} {} {}",
                    stills,
                    plural("still", stills),
                    films,
                    plural("filmstrip", films)
                ),
            )
            .claim(Component::Cap)
            .nowrap(),
        );
        let mut section = Element::surface()
            .gap(Self::GAP)
            .align_start()
            .width(inner)
            .child(cap);
        if !shelf.stills.is_empty() {
            let mut gallery = Element::row().wrap().gap(Self::THUMB_GAP).width(inner);
            for still in shelf.stills.iter().take(Self::THUMBS) {
                gallery = gallery.child(Element::picture(still.image.clone(), Self::THUMB_HEIGHT));
            }
            let more = shelf.stills.len().saturating_sub(Self::THUMBS);
            if more > 0 {
                gallery = gallery.child(
                    Element::text(Voice::CAP, lex.ov0, format!("+{more} more"))
                        .claim(Component::Cap)
                        .nowrap()
                        .height(Self::THUMB_HEIGHT),
                );
            }
            section = section.child(gallery);
        }
        let mut receipts = Element::surface().width(inner);
        receipts = receipts.child(ReceiptRow::value(
            (Icon::Commit, "commit"),
            &shelf.commit_text(),
            None,
            true,
            ground,
            &lex,
        ));
        receipts = receipts.child(if shelf.gates.is_empty() {
            ReceiptRow::value((Icon::Gate, "gates"), "none kept", None, true, ground, &lex)
        } else {
            let lit: Vec<bool> = shelf.gates.iter().map(|(_, passed)| *passed).collect();
            ReceiptRow::leds((Icon::Gate, "gates"), &lit, None, true, ground, &lex)
        });
        receipts = receipts.child(ReceiptRow::value(
            (Icon::Cost, "cost"),
            &shelf.cost_text(),
            None,
            true,
            ground,
            &lex,
        ));
        receipts = receipts.child(ReceiptRow::value(
            (Icon::Trace, "trace"),
            &shelf.trace_text(),
            None,
            true,
            ground,
            &lex,
        ));
        let bundled = shelf.bundle.is_some();
        receipts = receipts.child(ReceiptRow::value(
            (Icon::Session, "ship"),
            &shelf.ship_text(),
            None,
            bundled,
            ground,
            &lex,
        ));
        if let Some(bundle) = &shelf.bundle {
            receipts = receipts.child(ReceiptRow::value(
                (Icon::Folder, "bundle"),
                &tail(bundle),
                None,
                false,
                ground,
                &lex,
            ));
        }
        section = section.child(receipts);
        section
            .child(ActionRow::new(Some(Action::new("Export", Self::EXPORT)), Vec::new()).el(medium))
    }
}

/// `word` or `words`, by `n` — the copy law's numerals want a noun that
/// agrees with them.
fn plural(word: &str, n: usize) -> String {
    if n == 1 {
        word.to_owned()
    } else {
        format!("{word}s")
    }
}

/// The last three components of `path` — `.magistr/deliverables/<chunk>`
/// — which is where a bundle goes and all a value slot has room for.
fn tail(path: &std::path::Path) -> String {
    let parts: Vec<String> = path
        .components()
        .rev()
        .take(3)
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    parts.into_iter().rev().collect::<Vec<_>>().join("/")
}

#[cfg(test)]
#[path = "comp_shelf.tests.rs"]
mod tests;
