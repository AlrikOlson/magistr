//! Flexbox layout over the cell grid — or over a window's pixels, from
//! the same tree.
//!
//! Describe the screen as a tree of [`Node`]s — rows, columns, leaves and
//! wrapping text — with [`Length`] constraints, hand it to a
//! [`LayoutTree`], and get back a [`Resolved`] list of [`Rect`]s in cells,
//! one per node, keyed by [`WidgetId`](crate::WidgetId) for painting and
//! hit-testing. taffy does the flex arithmetic; it never appears in the
//! public surface.
//!
//! The engine is told what a cell is worth: a [`Basis`] gives the size of
//! one cell along each axis and a [`Measure`] answers how wide a string
//! is and how tall its rows are. [`LayoutTree::compute`] is the terminal's
//! pair — [`Basis::CELLS`] and [`CellMeasure`], where every conversion is
//! the identity — and [`LayoutTree::compute_on`] takes the window's, where
//! a cell is 9x20 pixels and a heading row is taller than a label row.
//! Every [`Placed`] then carries both readings: the exact [`PxRect`] the
//! engine produced and the [`Rect`] of whole cells it rounds to.

pub mod align;
pub mod basis;
pub mod cell_measure;
pub mod error;
pub mod extent;
pub mod frame;
pub mod inset;
pub mod justify;
pub mod length;
pub mod measure;
pub mod node;
pub mod placed;
pub mod px_rect;
pub mod rect;
pub mod resolved;
pub mod text;
pub mod text_role;
pub mod tree;

pub use align::Align;
pub use basis::Basis;
pub use cell_measure::CellMeasure;
pub use error::LayoutError;
pub use extent::Extent;
pub use frame::Frame;
pub use inset::Inset;
pub use justify::Justify;
pub use length::Length;
pub use measure::Measure;
pub use node::Node;
pub use placed::Placed;
pub use px_rect::PxRect;
pub use rect::Rect;
pub use resolved::Resolved;
pub use text::TextMetrics;
pub use text_role::TextRole;
pub use tree::LayoutTree;
