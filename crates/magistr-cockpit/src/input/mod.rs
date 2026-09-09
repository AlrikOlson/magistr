//! Terminal input as typed events, decoded from bytes.
//!
//! [`Decoder::feed`] takes whatever the terminal wrote — in any
//! fragmentation — and yields [`Event`]s: keys (legacy encodings and the
//! kitty keyboard protocol), SGR mouse reports, bracketed paste, focus
//! changes and the replies to the cockpit's startup queries. Nothing here
//! reads a file descriptor; the runtime's input thread moves the bytes.

pub mod button;
pub mod csi;
pub mod decoder;
pub mod event;
pub mod key_code;
pub mod key_event;
pub mod key_kind;
pub mod modifiers;
pub mod mouse_event;
pub mod mouse_kind;
pub mod reply;

pub use button::Button;
pub use decoder::Decoder;
pub use event::Event;
pub use key_code::KeyCode;
pub use key_event::KeyEvent;
pub use key_kind::KeyKind;
pub use modifiers::Modifiers;
pub use mouse_event::MouseEvent;
pub use mouse_kind::MouseKind;
pub use reply::Reply;
