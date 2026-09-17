//! One node of the roadmap's tech-tree, placed on the cell grid.

use super::chunk_state::ChunkState;
use super::star_state::StarState;

/// A chunk placed by [`Roadmap::dag_nodes`](super::Roadmap::dag_nodes):
/// cell-space coordinates inside the graph block (pan already applied,
/// so a node can sit outside the visible rect), the status that picks
/// its colour, and the marks the painter rings — "you are here" and the
/// selection.
#[derive(Clone, Debug, PartialEq)]
pub struct DagNode {
    /// Column-cell x inside the graph block.
    pub x: f32,
    /// Row-cell y inside the graph block.
    pub y: f32,
    /// The chunk's state, for the colour.
    pub status: ChunkState,
    /// What the chunk is doing, in the sky's own vocabulary — the
    /// decoration the native constellation reads. The cell painter
    /// keeps reading `status`: a dot on a terminal has one colour and
    /// no room for seven states.
    pub state: StarState,
    /// The workstream's ordinal, for the region tint.
    pub region: usize,
    /// Whether this chunk is in progress — "you are here".
    pub here: bool,
    /// Whether this chunk is the selected one.
    pub selected: bool,
}
