use accesskit::{Action, NodeId, Role};

/// One published accessibility node, in a shape a test can read.
///
/// This representation keeps the model target alongside the platform semantics
/// for request validation and renderer tests.
#[derive(Clone, Debug, PartialEq)]
pub struct AccessNode {
    /// Stable for this control's lifetime; never reused for another object.
    pub id: NodeId,
    /// Current semantic input target; node IDs are never decoded as row positions.
    pub target: Option<magistr_cockpit::widget_id::WidgetId>,
    pub(crate) identity: String,
    /// What kind of thing it is, to assistive technology.
    pub role: Role,
    /// What it is called.
    pub label: String,
    /// What it currently holds, for the widgets that hold something —
    /// editable text and the conversation's visible text.
    pub value: Option<String>,
    /// Its rect in **physical** window pixels: `(x0, y0, x1, y1)`. `None`
    /// for the window itself, which has no rect of its own.
    pub bounds: Option<(f64, f64, f64, f64)>,
    /// The nodes directly under it, in layout order.
    pub children: Vec<NodeId>,
    /// Supported actions, derived from the rendered control.
    pub actions: Vec<Action>,
    /// Unavailable controls remain readable but cannot be activated.
    pub disabled: bool,
}
