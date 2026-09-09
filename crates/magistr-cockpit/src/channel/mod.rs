//! The two channels between the core and whoever drives it: the
//! [`Inbox`] every producer posts a [`Msg`](crate::cockpit::Msg) on, and
//! the [`Commands`] a driver takes the core's [`Cmd`](crate::cockpit::Cmd)s
//! from. Tokio unbounded channels underneath, with an optional tap (a
//! second observer of everything sent) and an optional stamp
//! ([`Arrivals`], the instant of every send, for event-to-paint
//! measurements).
//!
//! Nothing here knows what the driver is: the terminal runtime, the
//! native window and the iteration loop all hold the same two ends.

pub mod arrivals;
pub mod commands;
pub mod inbox;

pub use arrivals::Arrivals;
pub use commands::Commands;
pub use inbox::Inbox;
