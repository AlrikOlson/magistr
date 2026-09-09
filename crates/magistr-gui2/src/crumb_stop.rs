use crate::element_id::ElementId;
use crate::mark::Mark;

/// One stop of a [`Crumb`](crate::crumb::Crumb) trail: its label, the
/// mark drawn before it (the galaxy for `projects`, a project's sigil
/// for the project itself), whether it is the place the user is *at*
/// (`.c.here` — full ink, and never a link), and the id it answers on
/// when it is still a place you can go back to.
pub struct CrumbStop {
    /// The place's name, in the volume's own copy.
    pub label: String,
    /// The mark before the label, when the stop carries one.
    pub mark: Option<Mark<'static>>,
    /// Whether this stop is where the user is — emphasized, inert.
    pub here: bool,
    /// The id a navigable stop answers on. `None` on the `here` stop.
    pub id: Option<ElementId>,
}

impl CrumbStop {
    /// A stop you can navigate back to.
    #[must_use]
    pub fn back(label: impl Into<String>, mark: Option<Mark<'static>>, id: ElementId) -> Self {
        Self {
            label: label.into(),
            mark,
            here: false,
            id: Some(id),
        }
    }

    /// The stop the user is at — full ink, no id, no pointer.
    #[must_use]
    pub fn here(label: impl Into<String>, mark: Option<Mark<'static>>) -> Self {
        Self {
            label: label.into(),
            mark,
            here: true,
            id: None,
        }
    }
}
