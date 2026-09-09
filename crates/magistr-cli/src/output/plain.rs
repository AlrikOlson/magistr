//! Plain text renderer with colored terminal output.
//!
//! Uses `utils::format` for consistent styling. Respects `NO_COLOR`.

use std::io::Write;

use crate::utils::format as fmt;

use super::renderer::{DataRenderer, OutputRenderer, TextRenderer};

/// Colored terminal output renderer.
///
/// Produces human-readable output with ANSI colors (unless `NO_COLOR` is set).
/// Messages go to the provided writer (typically stderr for interactive use).
#[derive(Debug, Default)]
pub struct PlainRenderer;

impl PlainRenderer {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl TextRenderer for PlainRenderer {
    fn success(&self, w: &mut dyn Write, msg: &str) -> std::io::Result<()> {
        writeln!(w, "{}", fmt::success(msg))
    }

    fn error(&self, w: &mut dyn Write, msg: &str) -> std::io::Result<()> {
        writeln!(w, "{}", fmt::error(msg))
    }

    fn warning(&self, w: &mut dyn Write, msg: &str) -> std::io::Result<()> {
        writeln!(w, "{}", fmt::warning(msg))
    }

    fn info(&self, w: &mut dyn Write, msg: &str) -> std::io::Result<()> {
        writeln!(w, "{}", fmt::info(msg))
    }
}

impl DataRenderer for PlainRenderer {
    fn table(
        &self,
        w: &mut dyn Write,
        headers: &[String],
        rows: &[Vec<String>],
    ) -> std::io::Result<()> {
        if headers.is_empty() {
            return Ok(());
        }

        // Calculate column widths
        let mut widths: Vec<usize> = headers.iter().map(String::len).collect();
        for row in rows {
            for (i, cell) in row.iter().enumerate() {
                if i < widths.len() && cell.len() > widths[i] {
                    widths[i] = cell.len();
                }
            }
        }

        // Print header
        let header_line: String = headers
            .iter()
            .enumerate()
            .map(|(i, h)| format!("{:<width$}", h, width = widths[i]))
            .collect::<Vec<_>>()
            .join("  ");
        writeln!(w, "  {}", fmt::bold(&header_line))?;

        // Print separator
        let sep: String = widths
            .iter()
            .map(|&w| "─".repeat(w))
            .collect::<Vec<_>>()
            .join("──");
        writeln!(w, "  {}", fmt::dim(&sep))?;

        // Print rows
        for row in rows {
            let line: String = row
                .iter()
                .enumerate()
                .map(|(i, cell)| {
                    let width = widths.get(i).copied().unwrap_or(cell.len());
                    format!("{:<width$}", cell, width = width)
                })
                .collect::<Vec<_>>()
                .join("  ");
            writeln!(w, "  {line}")?;
        }

        Ok(())
    }

    fn json(&self, w: &mut dyn Write, data: &serde_json::Value) -> std::io::Result<()> {
        // In plain mode, pretty-print JSON data
        let formatted = serde_json::to_string_pretty(data).map_err(std::io::Error::other)?;
        writeln!(w, "{formatted}")
    }

    fn kv(&self, w: &mut dyn Write, pairs: &[(String, String)]) -> std::io::Result<()> {
        for (key, value) in pairs {
            writeln!(w, "  {}", fmt::kv(key, &value))?;
        }
        Ok(())
    }

    fn list(&self, w: &mut dyn Write, items: &[String]) -> std::io::Result<()> {
        for item in items {
            writeln!(w, "  {item}")?;
        }
        Ok(())
    }
}

impl OutputRenderer for PlainRenderer {}

#[cfg(test)]
#[path = "plain.tests.rs"]
mod tests;
