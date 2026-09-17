//! Surface-neutral key and pointer values used by shared interaction.
//!
//! Surfaces decode their own input protocols before constructing these values.

pub mod button;
pub mod key_code;
pub mod key_event;
pub mod key_kind;
pub mod modifiers;
pub mod mouse_event;
pub mod mouse_kind;

pub use button::Button;
pub use key_code::KeyCode;
pub use key_event::KeyEvent;
pub use key_kind::KeyKind;
pub use modifiers::Modifiers;
pub use mouse_event::MouseEvent;
pub use mouse_kind::MouseKind;
