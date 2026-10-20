//! Error types — canonical definitions live in `magistr-core`.
//!
//! This module re-exports `MagistrError` and `Result` from magistr-core,
//! then adds CLI-specific extensions that depend on `color_eyre` and `magistr_acp`.

pub use magistr_core::error::*;

/// Convert a `MagistrError` into an `eyre::Report` for command handlers.
///
/// The top-level CLI reporter owns suggestion attachment for both explicit
/// conversions and errors propagated with `?`. Decorating reports here would
/// print the same suggestion again when the command returns to that reporter.
pub fn into_report(err: MagistrError) -> color_eyre::eyre::Report {
    err.into()
}

/// Convert command results for reporting with suggestions at the CLI boundary.
pub trait ResultExt<T> {
    fn suggest(self) -> color_eyre::eyre::Result<T>;
}

impl<T> ResultExt<T> for Result<T> {
    fn suggest(self) -> color_eyre::eyre::Result<T> {
        self.map_err(into_report)
    }
}

#[cfg(test)]
#[path = "error.tests.rs"]
mod tests;
