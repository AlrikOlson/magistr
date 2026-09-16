//! The pixel chrome: widgets that paint RGBA through tiny-skia into a
//! [`PixelSurface`] sized in cells — the branch graph, the brand, the
//! gauge, the tech tree, the sparkline, the icons, the drawn panel ring,
//! the key-cap chip, the segmented control's segment, the rule and the
//! stream's images.
//!
//! A surface renders on demand: painting marks it dirty, and only a dirty
//! surface costs anything at present time. Who
//! presents a dirty surface, and how, is the renderer's: magistr-tui
//! ships it to the terminal through the kitty or sixel compositor,
//! magistr-gui2 uploads it as a textured quad.

pub mod branch_graph;
pub mod brand;
pub mod cell_size;
pub mod chip;
pub mod gauge;
pub mod icon;
pub mod image;
pub mod painter;
pub mod panel;
pub mod pixel_format;
pub mod roadmap_dag;
pub mod rounded;
pub mod rule;
pub mod segment;
pub mod sparkline;
pub mod surface;
pub mod typography;

pub use branch_graph::BranchGraph;
pub use brand::Brand;
pub use cell_size::CellSize;
pub use chip::Chip;
pub use gauge::Gauge;
pub use icon::Icon;
pub use image::Image;
pub use painter::Painter;
pub use panel::Panel;
pub use pixel_format::PixelFormat;
pub use roadmap_dag::RoadmapDag;
pub use rule::Rule;
pub use segment::Segment;
pub use sparkline::Sparkline;
pub use surface::PixelSurface;
pub use typography::Typography;
