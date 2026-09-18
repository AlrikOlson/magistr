use magistr_cockpit::cockpit::Msg;
use magistr_cockpit::input::{Button, KeyCode, Modifiers, MouseKind};

use crate::element::Element;
use crate::element_id::ElementId;
use crate::element_layout::ElementLayout;

/// Which element the pointer is over, which is being pressed, and
/// which holds keyboard focus.
///
/// # One event system, not two
///
/// This folds the cockpit's own [`Msg`] — the very messages the
/// window's input layer already produces and the cockpit's update loop
/// already runs. A pointer message carries its physical pixel, so the
/// hit is resolved against the boxes the element layout actually
/// placed; a Tab press walks the focusable elements in tree order.
/// Nothing new is on the wire, which is what "no second event system"
/// has to mean.
///
/// # Why state lives here and not on the [`Element`]
///
/// The element tree is declarative and rebuilt per frame; hover, press
/// and focus outlive any one build. They are keyed by [`ElementId`],
/// so a rebuilt tree wearing the same ids keeps its interaction state
/// — the same reason the retained-tree toolkits key their hot/active
/// pair by widget identity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ElementState {
    hover: Option<ElementId>,
    pressed: Option<ElementId>,
    focus: Option<ElementId>,
}

impl ElementState {
    /// Fold one cockpit message over the placed tree. Answers whether
    /// anything changed — the caller's cue to paint.
    pub fn fold(&mut self, msg: &Msg, root: &Element, layout: &ElementLayout) -> bool {
        let before = *self;
        match msg {
            Msg::Mouse { event, .. } => {
                if let Some((x, y)) = event.px {
                    #[allow(clippy::cast_precision_loss)]
                    let at = (x as f32, y as f32);
                    match event.kind {
                        MouseKind::Move | MouseKind::Drag(_) => {
                            self.hover = hit(root, layout, at, |element| element.id.is_some());
                        }
                        MouseKind::Down(Button::Left) => {
                            self.hover = hit(root, layout, at, |element| element.id.is_some());
                            self.pressed = self.hover;
                            self.focus = hit(root, layout, at, |element| element.focusable);
                        }
                        MouseKind::Up(Button::Left) => self.pressed = None,
                        _ => {}
                    }
                }
            }
            Msg::Key(key) if key.is_press() && key.code == KeyCode::Tab => {
                let order = focus_order(root, layout);
                if order.is_empty() {
                    self.focus = None;
                } else if key.mods.contains(Modifiers::SHIFT) {
                    // Backwards enters the ring at its far end.
                    self.focus = Some(step(&order, self.focus, order.len() - 1, |i| {
                        i.checked_sub(1).unwrap_or(order.len() - 1)
                    }));
                } else {
                    self.focus = Some(step(&order, self.focus, 0, |i| (i + 1) % order.len()));
                }
            }
            // ↑/↓ walk the walkable run — a feed's card heads — from
            // the one that holds focus to its neighbour in tree order,
            // and stop at either end. With focus elsewhere the arrows
            // are not this state's, and the caller passes them on.
            Msg::Key(key) if key.is_press() && matches!(key.code, KeyCode::Up | KeyCode::Down) => {
                let walk = walk_order(root, layout);
                if let Some(at) = self
                    .focus
                    .and_then(|id| walk.iter().position(|&other| other == id))
                {
                    let next = if key.code == KeyCode::Up {
                        at.checked_sub(1)
                    } else {
                        (at + 1 < walk.len()).then_some(at + 1)
                    };
                    if let Some(next) = next {
                        self.focus = Some(walk[next]);
                    }
                }
            }
            _ => {}
        }
        *self != before
    }

    /// Whether the arrow keys are this state's to spend: focus is on a
    /// walkable element of `root`.
    #[must_use]
    pub fn walks(&self, root: &Element, layout: &ElementLayout) -> bool {
        self.focus
            .is_some_and(|id| walk_order(root, layout).contains(&id))
    }

    /// The element under the pointer, if it wears an id.
    #[must_use]
    pub fn hover(&self) -> Option<ElementId> {
        self.hover
    }

    /// The element a press is currently held on.
    #[must_use]
    pub fn pressed(&self) -> Option<ElementId> {
        self.pressed
    }

    /// The element holding keyboard focus.
    #[must_use]
    pub fn focus(&self) -> Option<ElementId> {
        self.focus
    }
}

/// The topmost element at `at` that `wants` it and is not disabled —
/// topmost meaning latest in tree order, which is paint order.
fn hit(
    root: &Element,
    layout: &ElementLayout,
    at: (f32, f32),
    wants: impl Fn(&Element) -> bool,
) -> Option<ElementId> {
    let clips = crate::frame_clip::FrameClips::of(root, layout);
    let mut found = None;
    root.each(&mut |index, element| {
        if element.disabled || !wants(element) {
            return;
        }
        let Some([x, y, w, h]) = clips.visible(index, layout.rect(index)) else {
            return;
        };
        if at.0 >= x && at.0 < x + w && at.1 >= y && at.1 < y + h {
            found = element.id;
        }
    });
    found
}

/// Every focusable, enabled element's id, in tree order — the Tab
/// ring.
fn focus_order(root: &Element, layout: &ElementLayout) -> Vec<ElementId> {
    let clips = crate::frame_clip::FrameClips::of(root, layout);
    let mut order = Vec::new();
    root.each(&mut |index, element| {
        if clips.visible(index, layout.rect(index)).is_none() {
            return;
        }
        if element.focusable && !element.disabled {
            order.extend(element.id);
        }
    });
    order
}

/// Every walkable, enabled element's id, in tree order — the run the
/// arrows move along.
fn walk_order(root: &Element, layout: &ElementLayout) -> Vec<ElementId> {
    let clips = crate::frame_clip::FrameClips::of(root, layout);
    let mut order = Vec::new();
    root.each(&mut |index, element| {
        if clips.visible(index, layout.rect(index)).is_none() {
            return;
        }
        if element.walkable && !element.disabled {
            order.extend(element.id);
        }
    });
    order
}

/// The next stop on the Tab ring: from `current` via `advance`, or the
/// ring's entry stop when nothing holds focus yet.
fn step(
    order: &[ElementId],
    current: Option<ElementId>,
    entry: usize,
    advance: impl Fn(usize) -> usize,
) -> ElementId {
    current
        .and_then(|id| order.iter().position(|&other| other == id))
        .map_or(order[entry], |i| order[advance(i)])
}

#[cfg(test)]
#[path = "element_state.tests.rs"]
mod tests;
