//! One stop of the window's altitude path.

use super::widget::Widget;

/// A breadcrumb derived by [`Model::path`](super::Model::path) from the
/// current [`Place`](super::Place). It carries a widget, label, and whether
/// it names the current place or a clickable ancestor.
///
/// Renderers display these stops without deciding the navigation path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathStop {
    /// The crumb that answers a click on this stop.
    pub widget: Widget,
    /// The name the stop wears.
    pub label: String,
    /// Whether this is the place the reader is at.
    pub here: bool,
}

impl PathStop {
    /// An ancestor stop: presses, climbs to `widget`'s place.
    pub(super) fn back(widget: Widget, label: impl Into<String>) -> Self {
        Self {
            widget,
            label: label.into(),
            here: false,
        }
    }

    /// The here stop: emphasized, inert.
    pub(super) fn here(widget: Widget, label: impl Into<String>) -> Self {
        Self {
            widget,
            label: label.into(),
            here: true,
        }
    }
}
