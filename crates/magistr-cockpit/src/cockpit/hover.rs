//! The pointer's answer: whatever it is over is raised, and whatever it
//! is holding is pressed.

use super::hover_role::HoverRole;
use crate::layout::Node;
use crate::render::Style;
use crate::theme::Pen;
use crate::widget_id::WidgetId;

/// Give the widget carrying `hovered` its [`HoverRole`]'s treatment —
/// held down when it is also the `pressed` one — and report whether
/// anything on screen changed.
///
/// One pass over the very tree the hit registry is built from
/// ([`HitRegistry::register_layout`](crate::hit::HitRegistry::register_layout)
/// walks the same resolved layout), so "every hittable answers the
/// pointer" is a property of the pass rather than of forty call sites
/// remembering to ask. Paint only: no node is added, removed or
/// re-measured, so a hovered frame lays out cell for cell like the frame
/// beneath it.
///
/// The press is composed *onto* the hover treatment rather than being a
/// fourth role of its own, because a press is not a different kind of
/// widget — it is the same widget, held. That keeps
/// [`HoverRole`]'s partition the one thing deciding how a widget answers
/// the pointer, and it means an [`HoverRole::Inert`] widget stays silent
/// under the button exactly as it is under the pointer: tinting a panel
/// because a click landed in its padding would be noise.
pub fn answer(
    root: &mut Node,
    hovered: Option<WidgetId>,
    pressed: Option<WidgetId>,
    pen: &Pen,
) -> bool {
    let Some(id) = hovered else {
        return false;
    };
    let treatment: &dyn Fn(&Style) -> Style = match HoverRole::of(id) {
        HoverRole::Surface => &|style| pen.raise(style),
        HoverRole::Lift => &|style| pen.lift(style),
        HoverRole::Link => &|style| pen.underline(style),
        HoverRole::Inert => return false,
    };
    // Only while the pointer is still on what it took hold of: a button
    // the pointer has dragged off is not being pressed, which is the
    // rule every platform's own controls follow.
    if pressed == Some(id) {
        return root.restyle_at(id, &|style| pen.press(&treatment(style)));
    }
    root.restyle_at(id, treatment)
}

#[cfg(test)]
#[path = "hover.tests.rs"]
mod tests;
