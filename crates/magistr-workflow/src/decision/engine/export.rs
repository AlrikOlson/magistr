//! Export the log in human-readable formats — markdown, console, JSON,
//! and the ASCII thread tree. Wraps `super::snapshots::history_with_threads`
//! and dispatches to the appropriate [`Formatter`](crate::decision::formatter::Formatter) method per format.

use crate::decision::config::OutputFormat;
use crate::decision::domain::Note;

use super::core::ReasoningServer;

impl ReasoningServer {
    /// ASCII art of the thread hierarchy.
    pub fn thread_tree(&self) -> String {
        let history = self.history_with_threads();
        self.formatter.format_thread_tree(&history)
    }

    /// Render the log in the requested output format.
    pub fn export_history(&self, format: OutputFormat) -> String {
        let snapshot = self.history_with_threads();
        match format {
            OutputFormat::Markdown => snapshot
                .notes
                .iter()
                .map(|s| self.formatter.format_note_markdown(s))
                .collect::<Vec<_>>()
                .join("\n\n"),
            OutputFormat::Console => snapshot
                .notes
                .iter()
                .map(|s| self.formatter.format_note_console(s))
                .collect::<Vec<_>>()
                .join("\n\n"),
            OutputFormat::Json => {
                serde_json::to_string_pretty(&snapshot).unwrap_or_else(|_| "{}".into())
            }
        }
    }

    /// Stderr-side single-note formatter used by `process_note` while
    /// recording. Format follows `config.display.output_format`.
    pub(crate) fn format_output(&self, note: &Note) -> String {
        match self.config.display.output_format {
            OutputFormat::Json => self.formatter.format_note_json(note),
            OutputFormat::Markdown => self.formatter.format_note_markdown(note),
            OutputFormat::Console => self.formatter.format_note_console(note),
        }
    }
}
