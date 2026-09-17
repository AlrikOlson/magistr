//! The mouse pointer's shape, as kitty's OSC 22 names it.

use super::hover_role::HoverRole;
use crate::widget_id::WidgetId;

/// The shape the terminal is asked to draw the mouse pointer as.
/// kitty's pointer-shape protocol takes W3C cursor names in
/// an OSC 22 string — `OSC 22; pointer ST` for the hand, `OSC 22; ST`
/// (an empty name) to hand the shape back to the terminal — so this is
/// maps controls and panel boundaries to their standard cursor names.
///
/// Only a surface that says so is asked
/// ([`Mode::Graphics`](crate::capability::Mode::Graphics)'s
/// `pointer_shapes`): the terminal runtime sets it for a terminal that
/// answered the kitty graphics query, since that terminal speaks the
/// pointer protocol too, and every other terminal — sixel included —
/// gets the colour change alone rather than a string it would print.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PointerShape {
    /// Whatever the terminal draws normally — the shape the cockpit
    /// hands back when the pointer leaves a control.
    #[default]
    Default,
    /// The pointing hand, over a button, a chip, a tab or a link.
    Hand,
    /// Resize adjacent columns.
    HorizontalResize,
    /// Resize stacked panels.
    VerticalResize,
}

impl PointerShape {
    /// The shape for the widget the pointer is over: a hand over
    /// anything clickable, the terminal's own shape everywhere else.
    #[must_use]
    pub fn over(hovered: Option<WidgetId>) -> Self {
        if let Some(divider) = hovered.and_then(super::flight_divider::FlightDivider::from_id) {
            return if divider == super::flight_divider::FlightDivider::Columns {
                Self::HorizontalResize
            } else {
                Self::VerticalResize
            };
        }
        match hovered.map(HoverRole::of) {
            Some(role) if role.is_clickable() => Self::Hand,
            _ => Self::Default,
        }
    }

    /// The OSC 22 sequence that asks for this shape. The default shape
    /// names nothing, which is how the protocol spells "reset".
    #[must_use]
    pub const fn sequence(self) -> &'static [u8] {
        match self {
            Self::Default => b"\x1b]22;\x1b\\",
            Self::Hand => b"\x1b]22;pointer\x1b\\",
            Self::HorizontalResize => b"\x1b]22;ew-resize\x1b\\",
            Self::VerticalResize => b"\x1b]22;ns-resize\x1b\\",
        }
    }
}

#[cfg(test)]
#[path = "pointer_shape.tests.rs"]
mod tests;
