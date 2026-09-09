//! What just happened, as far as the frame cadence cares.

/// The kind of thing the loop just folded into the model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Activity {
    /// A key, mouse report or paste from the user.
    Input,
    /// Anything else that changes the model: a resize, a focus change, a
    /// query reply, a message from the agent or the engines.
    Message,
}
