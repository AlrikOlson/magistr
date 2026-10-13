use accesskit::NodeId;

use crate::access_node::AccessNode;
use crate::element::{Element, Kind};
use crate::element_layout::ElementLayout;

/// The placed tree as accessibility nodes — one [`AccessNode`] per
/// element, in tree order, ids being the same tree indices layout and
/// emission use.
///
/// The label is the element's own when one was declared, and a text
/// run's content otherwise — a run *is* its text, and making a caller
/// restate it would be a second copy to drift. Bounds are the placed
/// pixel boxes, so what a screen reader is told an element covers is
/// exactly what was painted.
#[must_use]
pub fn access_of(root: &Element, layout: &ElementLayout) -> Vec<AccessNode> {
    let children = child_ids(root);
    let mut nodes = Vec::with_capacity(layout.len());
    root.each(&mut |index, element| {
        let [x, y, w, h] = layout.rect(index);
        let label = element
            .label
            .clone()
            .unwrap_or_else(|| match &element.kind {
                Kind::Text { content, .. } => content.clone(),
                // A picture says what it shows.
                Kind::Picture { image } => image.alt().to_owned(),
                // A mark with no declared label is decoration — the
                // meaning belongs to the copy beside it.
                // A terminal's rows are its own label, stamped at
                // construction ([`Element::grid`]).
                Kind::Surface | Kind::Mark { .. } | Kind::Grid { .. } => String::new(),
            });
        nodes.push(AccessNode {
            id: NodeId(index as u64),
            target: None,
            identity: String::new(),
            actions: Vec::new(),
            disabled: element.disabled,
            role: element.role,
            label,
            value: element.editor_value.clone(),
            bounds: Some((
                f64::from(x),
                f64::from(y),
                f64::from(x + w),
                f64::from(y + h),
            )),
            children: children.get(index).cloned().unwrap_or_default(),
        });
    });
    nodes
}

/// Every element's direct-child ids, computed in one depth-first walk
/// with the same numbering everything else uses.
fn child_ids(root: &Element) -> Vec<Vec<NodeId>> {
    fn walk(element: &Element, next: &mut usize, out: &mut Vec<Vec<NodeId>>) {
        let index = *next;
        *next += 1;
        out.push(Vec::new());
        for child in &element.children {
            out[index].push(NodeId(*next as u64));
            walk(child, next, out);
        }
    }
    let mut out = Vec::new();
    let mut next = 0;
    walk(root, &mut next, &mut out);
    out
}

#[cfg(test)]
#[path = "element_access.tests.rs"]
mod tests;
