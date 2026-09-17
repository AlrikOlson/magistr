use thiserror::Error;

/// What can go wrong between a token file and a window.
#[derive(Debug, Error)]
pub enum Gui2Error {
    /// The embedded mono face yields no usable grid at this size — a
    /// broken build rather than a runtime condition, but reported
    /// rather than panicked because this is a library.
    #[error("no cell grid from the embedded face at {0} px")]
    Metrics(f32),
    /// taffy refused the layout tree, which a tree built from `Node`s
    /// never does.
    #[error("layout: {0}")]
    Layout(#[from] magistr_cockpit::layout::LayoutError),
    /// taffy refused the element substrate's px tree, which a tree
    /// built from `Element`s never does. Its own variant rather than
    /// `Layout` because the substrate speaks no cockpit vocabulary —
    /// not even the error type's.
    #[error("element layout: {0}")]
    Elements(String),
    /// The CPU rasterizer refused the viewport (a zero side, or an atlas
    /// it cannot allocate).
    #[error("cpu rasterizer refused a {0}x{1} viewport")]
    Raster(u32, u32),
    /// A `NN.mask.json` under `tests/mocks` exists and cannot be read,
    /// or does not parse strictly — a mask mistyped into permissiveness
    /// would be a review by judgement wearing a gate's name.
    #[error("mask {path}: {error}")]
    Mask {
        /// The mask file.
        path: std::path::PathBuf,
        /// What was wrong with it.
        error: String,
    },
    /// winit could not build the event loop or the window.
    #[error("window: {0}")]
    Window(String),
    /// wgpu could not give us a surface or a device for it.
    #[error("gpu: {0}")]
    Gpu(String),
}
