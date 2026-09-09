//! Error types — canonical definitions live in `magistr-core`.
//!
//! This module re-exports `MagistrError` and `Result` from magistr-core,
//! then adds CLI-specific extensions that depend on `color_eyre` and `magistr_acp`.

pub use magistr_core::error::*;

/// Convert a `MagistrError` into an `eyre::Report` with an actionable suggestion section.
///
/// Use this instead of plain `.into()` when converting `MagistrError` to `eyre::Report`
/// at the application boundary (main.rs command handlers).
pub fn into_report(err: MagistrError) -> color_eyre::eyre::Report {
    use color_eyre::Section;
    let suggestion = err.suggestion().map(String::from);
    let report: color_eyre::eyre::Report = err.into();
    match suggestion {
        Some(text) => report.suggestion(text),
        None => report,
    }
}

/// Extension trait for converting `Result<T, MagistrError>` to `eyre::Result<T>` with suggestions.
///
/// Use `.suggest()?` instead of `?` at application boundaries (main.rs command handlers)
/// to attach actionable suggestions to errors.
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
