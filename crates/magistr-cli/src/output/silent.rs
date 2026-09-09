//! Silent renderer for CI mode.
//!
//! Produces no output. Commands communicate success/failure via exit codes only.
//! Useful for CI pipelines where only the exit code matters.

use std::io::Write;

use super::renderer::{DataRenderer, OutputRenderer, TextRenderer};

/// CI-mode renderer that produces no output.
///
/// Commands signal success/failure through their `Result` return type,
/// which translates to process exit codes.
#[derive(Debug, Default)]
pub struct SilentRenderer;

impl SilentRenderer {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl TextRenderer for SilentRenderer {
    fn success(&self, _w: &mut dyn Write, _msg: &str) -> std::io::Result<()> {
        Ok(())
    }

    fn error(&self, _w: &mut dyn Write, _msg: &str) -> std::io::Result<()> {
        Ok(())
    }

    fn warning(&self, _w: &mut dyn Write, _msg: &str) -> std::io::Result<()> {
        Ok(())
    }

    fn info(&self, _w: &mut dyn Write, _msg: &str) -> std::io::Result<()> {
        Ok(())
    }
}

impl DataRenderer for SilentRenderer {
    fn table(
        &self,
        _w: &mut dyn Write,
        _headers: &[String],
        _rows: &[Vec<String>],
    ) -> std::io::Result<()> {
        Ok(())
    }

    fn json(&self, _w: &mut dyn Write, _data: &serde_json::Value) -> std::io::Result<()> {
        Ok(())
    }

    fn kv(&self, _w: &mut dyn Write, _pairs: &[(String, String)]) -> std::io::Result<()> {
        Ok(())
    }

    fn list(&self, _w: &mut dyn Write, _items: &[String]) -> std::io::Result<()> {
        Ok(())
    }
}

impl OutputRenderer for SilentRenderer {}

#[cfg(test)]
#[path = "silent.tests.rs"]
mod tests;
