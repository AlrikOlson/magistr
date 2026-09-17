use accesskit::{NodeId, Role};

/// One published accessibility node, in a shape a test can read.
///
/// AccessKit's own `Node` is write-only from this side — it has setters
/// and no way to ask what a built node says — so the tree is recorded in
/// this plain form as well and converted on the way out. That is what
/// makes "the roles and names are the cockpit's own" a failing test
/// rather than a claim about code nobody can inspect. qs-ui's
/// `SemanticNode` keeps the same shape for the same reason.
#[derive(Clone, Debug, PartialEq)]
pub struct AccessNode {
    /// The node's id: the cockpit's [`WidgetId`] plus one, so the
    /// window can keep id 0 — `Widget::Header`'s raw id *is* 0.
    ///
    /// [`WidgetId`]: magistr_cockpit::widget_id::WidgetId
    pub id: NodeId,
    /// What kind of thing it is, to assistive technology.
    pub role: Role,
    /// What it is called.
    pub label: String,
    /// What it currently holds, for the widgets that hold something —
    /// the prompt's text, and nothing else.
    pub value: Option<String>,
    /// Its rect in **logical** window points: `(x0, y0, x1, y1)`. `None`
    /// for the window itself, which has no rect of its own.
    pub bounds: Option<(f64, f64, f64, f64)>,
    /// The nodes directly under it, in layout order.
    pub children: Vec<NodeId>,
}
