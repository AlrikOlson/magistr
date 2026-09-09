//! Fakes and fixtures that belong to the cockpit rather than to a
//! renderer: a scripted [`WorkbenchPort`](crate::workbench::WorkbenchPort)
//! for the tests that drive `update` through a workbench round-trip, and
//! the [`Scenes`] — the scripted models every still, filmstrip and view
//! test is taken of, so a terminal golden and a view assertion describe
//! the same model.
//!
//! The scripted terminal, agent, engines and input live in magistr-tui's
//! harness: they fake what a renderer talks to.

pub mod fake_workbench;
pub mod scenes;

pub use fake_workbench::FakeWorkbench;
pub use scenes::Scenes;
