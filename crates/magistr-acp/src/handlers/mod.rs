//! Handlers for the requests an agent makes of its client.
//!
//! Each function answers one agent→client method. They are plain async
//! functions so the connection builder can wrap them and the fake-agent tests
//! can call them directly; anything that waits on the outside world
//! (a permission prompt, a terminal's exit) is spawned off the dispatch loop
//! by the registration code in `transport::connection`.

pub(crate) mod effects;
pub(crate) mod fs;
pub(crate) mod permission;
pub(crate) mod terminal;
