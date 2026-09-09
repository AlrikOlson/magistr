//! The runtime: the loop that drives the Elm core over a [`Terminal`].
//!
//! [`Runtime::run`] takes the terminal over, runs the startup queries in
//! one raw-mode window, spawns the input thread, then loops over one
//! merged stream — the thread's channel, the
//! [`Inbox`](magistr_cockpit::channel::Inbox) every producer posts on,
//! and the frame tick — folding every message into the model and
//! painting only when something changed, at a cadence the
//! [`FrameScheduler`] picks from recent activity. Quit leaves the session
//! in reverse order and joins the thread.
//!
//! [`Terminal`]: crate::term::Terminal

pub mod activity;
mod driver;
pub mod error;
pub mod options;
mod osc52;
pub mod runner;
pub mod scheduler;
mod surfaces;
pub mod tier;

pub use activity::Activity;
pub use error::RuntimeError;
pub use options::Options;
pub use runner::Runtime;
pub use scheduler::FrameScheduler;
pub(crate) use surfaces::Surfaces;
pub use tier::Tier;
