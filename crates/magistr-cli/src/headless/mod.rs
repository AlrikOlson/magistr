//! Headless & CI mode — run magistr without the TUI.
//!
//! This module provides:
//! - `OutputFormat` — selects between JSON, plain text, and GitHub Actions output
//! - `ExitCode` — standardized process exit codes for CI pipelines
//! - `HeadlessEvent` — structured events emitted during headless sessions
//! - Three renderers: `JsonStreamRenderer`, `TextSummaryRenderer`, `GhaRenderer`

pub mod event;
pub mod exit_code;
pub mod format;
pub mod gha;
pub mod json_stream;
pub mod renderer;
pub mod text_summary;

pub use event::HeadlessEvent;
pub use exit_code::ExitCode;
pub use format::OutputFormat;
pub use gha::GhaRenderer;
pub use json_stream::JsonStreamRenderer;
pub use renderer::HeadlessRenderer;
pub use text_summary::TextSummaryRenderer;
