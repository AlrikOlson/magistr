//! One owned-process mechanism for every command Magistr runs on a caller's
//! behalf: the quality gates on their pty, the workflow's verified
//! `ship_check` commands, and the workspace's declared commands.
//!
//! What is shared is the mechanism, never the admission. A [`ProcessSpec`]
//! names what to launch — a shell line ([`Launch::Shell`]) or one literal
//! argv ([`Launch::Argv`]) — and every consumer decides which of the two it
//! is allowed to build: the gate runner passes the configured gate command,
//! the workflow passes the caller's command string, and only the workspace's
//! own command admission produces an argv. Nothing here turns a shell string
//! into an argv or the other way round.
//!
//! The child runs in its own session and process group with stdin closed and
//! both streams captured under a ceiling ([`BoundedCapture`]), on pipes or on
//! a fresh pty ([`OutputMode`]). A wall-clock timeout or a cancellation ends
//! the whole group — gracefully or at once ([`Termination`]) — and answers
//! partial output as data. Every chunk the child writes also reaches the
//! caller's sink as it lands, so a cockpit can stream a gate live. Dropping
//! an [`OwnedProcess`] mid-run kills its group and retains reaping ownership.
//! [`ProcessGroup`] couples group lifetime to [`ChildRegistry`]; cleanup only
//! succeeds after group disappearance. Admission remains with each caller.

#![warn(missing_docs)]

mod blocking;
mod bounded_capture;
pub use blocking::run_blocking;
mod child_registry;
mod launch;
mod output_mode;
mod owned_process;
#[cfg(windows)]
mod posix_shell;
#[cfg(not(windows))]
mod process_group;
#[cfg(windows)]
#[path = "process_group_windows.rs"]
mod process_group;
mod process_outcome;
mod process_spec;
#[cfg(unix)]
mod pty;
mod reaping_child;
mod strip_escapes;
mod termination;

pub use bounded_capture::BoundedCapture;
pub use child_registry::ChildRegistry;
pub use launch::Launch;
pub use output_mode::OutputMode;
pub use owned_process::OwnedProcess;
pub use process_group::ProcessGroup;
pub use process_outcome::ProcessOutcome;
pub use process_spec::ProcessSpec;
#[cfg(unix)]
pub use pty::Pty;
pub use reaping_child::ReapingChild;
pub use strip_escapes::EscapeStripper;
pub use termination::Termination;
