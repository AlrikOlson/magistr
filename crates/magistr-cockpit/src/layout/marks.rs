//! Every mark on a laid-out tree, with the rect layout gave its node.

use super::mark::Mark;
use super::node::Node;
use super::placed::Placed;
use super::rect::Rect;
use super::resolved::Resolved;

/// The marks of `root` after `resolved` laid it out, in tree order:
/// the drawn panel of every framed node whose frame is
/// [`drawn`](super::Frame::is_drawn), then the node's own marks — each
/// with the rect the node landed in. Nodes that landed nowhere (an
/// empty rect) carry nothing: there is no cell for a mark to cover.
///
/// The walk is the painter's own — parents before children, siblings in
/// layout order, one placed entry per node — so a mark lands where the
/// text it decorates was painted, never where a lookalike walk would
/// have put it.
#[must_use]
pub fn marks(root: &Node, resolved: &Resolved) -> Vec<(Rect, Mark)> {
    let mut out = Vec::new();
    let mut placed = resolved.placed().iter();
    collect(root, &mut placed, &mut out);
    out
}

fn collect(node: &Node, placed: &mut std::slice::Iter<'_, Placed>, out: &mut Vec<(Rect, Mark)>) {
    let Some(place) = placed.next() else {
        return;
    };
    let rect = place.rect;
    if !rect.is_empty() {
        if let Some(frame) = node.frame_spec().filter(|frame| frame.is_drawn()) {
            out.push((
                rect,
                Mark::Panel {
                    border: frame.border_style().fg,
                    title: frame.title_span(rect.width),
                },
            ));
        }
        out.extend(node.marks().iter().map(|mark| (rect, mark.clone())));
    }
    for child in node.child_nodes() {
        collect(child, placed, out);
    }
}

#[cfg(test)]
#[path = "marks.tests.rs"]
mod tests;
