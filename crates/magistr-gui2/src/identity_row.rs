use crate::component::Component;
use crate::element::Element;
use crate::lexicon::Lexicon;
use crate::mark::Mark;
use crate::voice::Voice;

/// The identity row — sigil · name · path —: a scope is named, never inferred. "This
/// project" is followed by the row that says which project, and a bare project name is
/// not an identity.
///
/// The volume draws it once, under Settings' `This project` (view 10: `gap:7px`, the
/// sigil at 14, the name sans 12/550, the path mono 9), and this is that row lifted out
/// so every place that names a scope draws the same one.
pub struct IdentityRow;

impl IdentityRow {
    /// The air between the sigil, the name and the path.
    pub const GAP: f32 = 7.0;
    /// The sigil's edge, in design pixels.
    pub const SIGIL_PX: u32 = 14;
    /// The name's voice — sans 12/550.
    pub const NAME: Voice = Voice {
        weight: 550,
        ..Voice::SLOT
    };
    /// The path's voice — mono 9.
    pub const PATH: Voice = Voice {
        px: 9.0,
        ..Voice::CHIP
    };

    /// One row: `sigil` when the project has one (a sigil is a pure
    /// function of a `'static` name — `Mark`'s contract — so a project
    /// named at runtime wears none), `name`, and `path` when known.
    #[must_use]
    pub fn el(
        sigil: Option<Mark<'static>>,
        name: &str,
        path: Option<&str>,
        lex: &Lexicon,
    ) -> Element {
        let mut row = Element::row().gap(Self::GAP).center();
        if let Some(mark) = sigil {
            row = row.child(Element::mark(
                mark,
                Self::SIGIL_PX,
                Lexicon::ink_bytes(lex.tx),
            ));
        }
        row = row.child(Element::text(Self::NAME, lex.tx, name).nowrap());
        if let Some(path) = path.filter(|path| !path.is_empty()) {
            row = row.child(Element::text(Self::PATH, lex.ov0, path).nowrap());
        }
        row.claim(Component::IdentityRow)
    }
}

#[cfg(test)]
#[path = "identity_row.tests.rs"]
mod tests;
