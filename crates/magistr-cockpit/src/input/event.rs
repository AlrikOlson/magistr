//! One decoded input event.

use super::key_event::KeyEvent;
use super::mouse_event::MouseEvent;
use super::reply::Reply;

/// What the terminal reported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// A key.
    Key(KeyEvent),
    /// A mouse report.
    Mouse(MouseEvent),
    /// Text pasted with bracketed paste, delivered whole.
    Paste(String),
    /// The terminal window gained (`true`) or lost focus.
    Focus(bool),
    /// An answer to a startup query.
    Reply(Reply),
}
