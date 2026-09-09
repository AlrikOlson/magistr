//! Mouse hit-testing: screen rects to widget ids.
//!
//! The cockpit registers the rects it paints into a [`HitRegistry`] each
//! frame (directly, or wholesale from a resolved layout) and asks it which
//! widget sits under a cell. Overlapping regions — a dock over the stage,
//! an overlay over everything — resolve to the topmost by stacking level.

pub mod registry;

pub use registry::HitRegistry;
