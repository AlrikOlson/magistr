use magistr_cockpit::cockpit::{Model, Place, Widget};

use crate::crumb::Crumb;
use crate::crumb_stop::CrumbStop;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::icon::Icon;
use crate::interned::interned;
use crate::lexicon::Lexicon;
use crate::mark::Mark;
use crate::medium::Medium;

/// The breadcrumb header — the altitude path, composed from the model: each ancestor a
/// pressed crumb stop and the place the reader is at the `here` stop. A composition
/// maps [`Model`] to component props and lays them out; it draws nothing of its own.
///
/// The path is the navigation: the stops are [`Model::path`], derived by the cockpit
/// from where the camera is — `projects` at the Galaxy, `projects › <project>` at the
/// Sky, `projects › <project> › <chunk>` from the Session down. The header decides only
/// the marks; it never decides where you are, so it cannot disagree with the spine.
pub struct HeaderComposition;

impl HeaderComposition {
    /// The `projects` stop — the climb to altitude 0.
    pub const PROJECTS: ElementId = ElementId(100);
    /// The project stop, pressable while a chunk sits below it.
    pub const PROJECT: ElementId = ElementId(101);
    /// The session stop, pressable from a record below it.
    pub const SESSION: ElementId = ElementId(102);

    /// The header's own padding — view 05's margin above the turn.
    const PAD_X: f32 = 18.0;
    const PAD_Y: f32 = 12.0;

    /// The path for `model`, in `medium`'s lexicon.
    #[must_use]
    pub fn el(model: &Model, medium: Medium) -> Element {
        let lex = Lexicon::of(medium);
        // The sigil is a pure function of a 'static name (`Mark`'s own
        // contract), so only the brand's default wears one — a session
        // title is dynamic and its stop goes unmarked until the sigil
        // learns to own its name.
        let brand = model.title().is_none();
        let stops: Vec<CrumbStop> = model
            .path()
            .into_iter()
            .map(|stop| {
                let (mark, id) = match stop.widget {
                    Widget::CrumbProjects => (Some(Mark::Icon(Icon::Projects)), Self::PROJECTS),
                    // A project being born wears its own sigil from
                    // the moment it has a name (the volume's F2–F5).
                    Widget::CrumbProject if model.place() == Some(Place::Birth) => (
                        Some(Mark::Sigil(interned(model.birth().name()))),
                        Self::PROJECT,
                    ),
                    Widget::CrumbProject => {
                        (brand.then_some(Mark::Sigil("magistr")), Self::PROJECT)
                    }
                    // The record stop is always the here stop, so its
                    // id is never placed; its mark is the record's own
                    // glyph — the trace's (view 11), the signal beacon
                    // for the inbox (view 12), and none for the Changes
                    // journal until a view of it is built.
                    Widget::CrumbRecord => (
                        match model.place() {
                            Some(Place::Trace) => Some(Mark::Icon(Icon::Trace)),
                            Some(Place::Signals) => Some(Mark::Icon(Icon::Signal)),
                            _ => None,
                        },
                        Self::SESSION,
                    ),
                    _ => (None, Self::SESSION),
                };
                if stop.here {
                    CrumbStop::here(stop.label, mark)
                } else {
                    CrumbStop::back(stop.label, mark, id)
                }
            })
            .collect();
        Element::row()
            .pad_x(Self::PAD_X)
            .pad_y(Self::PAD_Y)
            .center()
            .child(Crumb::el(&stops, &lex))
    }
}

#[cfg(test)]
#[path = "comp_header.tests.rs"]
mod tests;
