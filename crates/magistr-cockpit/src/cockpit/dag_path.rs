//! One dependency route in cell coordinates.

/// A dependency's endpoints and ordered routing points, including both nodes.
#[derive(Clone, Debug, PartialEq)]
pub struct DagPath {
    /// Source node in graph order.
    pub from: usize,
    /// Destination node in graph order.
    pub to: usize,
    /// Cell-space points, with the camera transform already applied.
    pub points: Vec<(f32, f32)>,
}
