//! Terminal input as typed events, decoded from bytes.
//!
//! [`Decoder::feed`] takes whatever the terminal wrote — in any
//! fragmentation — and yields [`Event`]s: keys (legacy encodings and the
//! kitty keyboard protocol), SGR mouse reports, bracketed paste, focus
//! changes and the replies to the cockpit's startup queries. Nothing here
//! reads a file descriptor; the runtime's input thread moves the bytes.

mod csi;
mod decoder;
mod event;
mod reply;

pub use decoder::Decoder;
pub use event::Event;
pub use reply::Reply;
