//! The pixel layer's wire side: the two graphics protocols as bytes — the
//! kitty protocol and sixel — tmux DCS passthrough, and the startup probe
//! that decides which of them the cockpit runs on, or whether it runs as
//! plain text.
//!
//! Nothing here touches a terminal. Every command is appended to a
//! `Vec<u8>` and every reply is parsed from a byte slice, so the exact wire
//! format is unit-tested and the driver that owns raw mode (the spike, then
//! the runtime) only moves bytes.

pub mod decision;
pub mod env;
pub mod error;
pub mod image_id;
pub mod kitty;
pub mod palette;
pub mod passthrough;
pub mod placement_id;
pub mod probe;
pub mod probe_reply;
pub mod protocol;
pub mod sixel;

pub use decision::Decision;
pub use env::Env;
pub use error::GraphicsError;
pub use image_id::ImageId;
pub use kitty::Kitty;
pub use palette::Palette;
pub use passthrough::Passthrough;
pub use placement_id::PlacementId;
pub use probe::Probe;
pub use probe_reply::ProbeReply;
pub use protocol::Protocol;
