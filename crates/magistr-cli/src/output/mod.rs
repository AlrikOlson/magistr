//! Output rendering system.
//!
//! Three traits form the renderer hierarchy:
//! - [`TextRenderer`] — message-level output (success, error, warning, info)
//! - [`DataRenderer`] — structured data (table, json, key-value, list)
//! - [`OutputRenderer`] — combines both + format metadata + `CommandResult` dispatch
//!
//! Three implementations:
//! - `PlainRenderer` — colored terminal output (default)
//! - `JsonRenderer` — machine-readable JSON with envelope format
//! - `SilentRenderer` — CI mode, no output (exit codes only)
//!
//! Command handlers accept `&dyn OutputRenderer` and return `CommandResult`
//! instead of performing I/O directly.

pub mod annotations;
mod command_result;
pub mod envelope;
mod json;
mod plain;
pub mod pr_comment;
pub mod renderer;
mod silent;

// Re-export the primary public types.
// Some re-exports are not yet consumed outside this module but form the
// public API surface — suppress dead-code warnings for incremental wiring.
pub use command_result::CommandResult;
pub use envelope::{ValidationOutput, success};
pub use json::JsonRenderer;
pub use plain::PlainRenderer;
pub use renderer::{DataRenderer, OutputRenderer, TextRenderer};
pub use silent::SilentRenderer;

// Public API re-exports not yet consumed by command handlers.
// Will be wired incrementally as handlers are converted to CommandResult.
#[allow(unused_imports)]
pub use envelope::{Envelope, ErrorEnvelope, GateOutput, error};

/// Create the appropriate renderer based on CLI flags.
///
/// Priority: `--json` > `--silent` > default (plain).
#[must_use]
pub fn create_renderer(json: bool, silent: bool) -> Box<dyn OutputRenderer> {
    if json {
        Box::new(JsonRenderer::new())
    } else if silent {
        Box::new(SilentRenderer::new())
    } else {
        Box::new(PlainRenderer::new())
    }
}

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
