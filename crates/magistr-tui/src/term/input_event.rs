//! What the input thread sends the runtime.

use magistr_cockpit::cockpit::Size;
use magistr_cockpit::input::Event;

/// One item on the input thread's channel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputEvent {
    /// A decoded terminal event.
    Event(Event),
    /// The window size changed (polled after a quiet read).
    Resize(Size),
    /// The input side closed or failed; nothing more will arrive and the
    /// thread has exited.
    Closed,
}
