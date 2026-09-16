//! One dependency route in the graph's camera coordinates.

/// A dependency's endpoints and ordered routing points, including both nodes.
#[derive(Clone, Debug, PartialEq)]
pub struct DagPath {
    /// Source node in graph order.
    pub from: usize,
    /// Destination node in graph order.
    pub to: usize,
    /// Points with the camera transform applied: pixels for a physical graph,
    /// logical cells for a legacy graph.
    pub points: Vec<(f32, f32)>,
}
