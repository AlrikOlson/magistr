//! The pixel layer's terminal transport: the compositors that ship the
//! cockpit's pixel surfaces
//! ([`PixelSurface`](magistr_cockpit::pixel::PixelSurface), painted by
//! the chrome in magistr-cockpit) to the terminal inside the cell frame's
//! synchronized-output bracket.
//!
//! A surface renders on demand: painting marks it dirty, and only a dirty
//! surface costs bytes at present time.
//!
//! Which compositor does the shipping is [`Backend`]'s decision, made once
//! from the probe's protocol. Under kitty each surface holds two image
//! ids from the session's own [`IdSpace`] (the terminal's id namespace
//! can be shared with another cockpit, under tmux for one); a dirty
//! frame transmits the idle one, places it under the
//! surface's placement id and deletes the previous placement, so there is
//! never a moment without an image on screen. Under sixel there are no ids
//! at all: a dirty or moved surface is re-emitted whole at its top-left
//! cell, and a surface that leaves tells the [`Screen`](crate::render::Screen)
//! to repaint the cells it covered.

pub mod backend;
pub mod compositor;
pub mod compression;
pub mod id_space;
pub mod sixel_compositor;

pub use backend::Backend;
pub use compositor::Compositor;
pub use compression::Compression;
pub use id_space::IdSpace;
pub use sixel_compositor::SixelCompositor;
