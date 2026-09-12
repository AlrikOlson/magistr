use super::{
    ReadInput, ReadRange, ReadRow, ReadSegment, ReadSurround, ReadUnit, WorkspaceQueries,
    source_observation::SourceObservation,
};
use crate::{
    contracts::{EnvelopeVersion, ErrorKind, WorkspaceError, WorkspaceResponse},
    file_content::FileContent,
    index::{IndexQuery, cache::IndexedFile},
};
use regex::Regex;
use std::sync::OnceLock;

impl WorkspaceQueries<'_> {
    /// Read original source bytes as bounded text with verified line anchors.
    /// Only symbol addressing requires a completed derived index.
    pub async fn read(&self, input: &ReadInput) -> WorkspaceResponse<ReadRow> {
        run(self, input).await
    }
}

pub(super) async fn run(q: &WorkspaceQueries<'_>, input: &ReadInput) -> WorkspaceResponse<ReadRow> {
    if let Some(error) = invalid_selection(input) {
        return super::budget::failure(error);
    }
    let snapshot = match q.snapshot(&input.file) {
        Ok(snapshot) => snapshot,
        Err(error) => return super::budget::failure(error),
    };
    // Byte classification precedes index access so binary files always receive
    // a text refusal, including while the derived index is unavailable.
    let outline = if input.symbol.is_some() && matches!(snapshot.content, FileContent::Text(_)) {
        match q.index_file(&snapshot.path) {
            Ok(outline) => Some(outline),
            Err(error) => return super::budget::failure(error),
        }
    } else {
        None
    };
    render(snapshot, input, outline)
}

fn render(
    snapshot: SourceObservation,
    input: &ReadInput,
    outline: Option<IndexQuery<IndexedFile>>,
) -> WorkspaceResponse<ReadRow> {
    if let Some(error) = invalid_selection(input) {
        return super::budget::failure(error);
    }
    let FileContent::Text(content) = &snapshot.content else {
        return super::budget::failure(WorkspaceError::new(
            ErrorKind::PatternInvalid,
            format!(
                "{} contains binary bytes or invalid UTF-8; text read cannot preserve its bytes",
                snapshot.path
            ),
        ));
    };
    let mut lines: Vec<_> = content.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    let selection =
        match super::read_selection::ReadSelection::resolve(&snapshot, input, &lines, outline) {
            Ok(selection) => selection,
            Err((error, notes)) => return failure_notes(error, notes),
        };
    let unit = ReadUnit {
        file: snapshot.path.clone(),
        range: selection.range,
        total_lines: lines.len(),
        content_hash: snapshot.content_hash,
        symbol: selection.symbol,
        enclosing: selection.enclosing,
    };
    let mut notes = selection.notes;
    let rows = content_rows(unit, &lines, input.surround, &mut notes);
    super::budget::success(
        rows,
        input.budget,
        selection.freshness,
        notes,
        |row| match row {
            ReadRow::Unit(_) => "-".into(),
            ReadRow::Imports(_) => "imports".into(),
            ReadRow::Lines(segment) => format!("line:{}", segment.start),
        },
        vec![
            "lines: {start, end} a narrower range".into(),
            "surround: \"none\"".into(),
            "raise budget".into(),
        ],
    )
}

fn content_rows(
    unit: ReadUnit,
    lines: &[&str],
    surround: ReadSurround,
    notes: &mut Vec<String>,
) -> Vec<ReadRow> {
    let range = unit.range;
    let file = unit.file.clone();
    let mut rows = vec![ReadRow::Unit(unit)];
    if surround == ReadSurround::Imports && !lines.is_empty() {
        match import_block(lines) {
            None => notes
                .push("surround:imports found no import lines in the leading scan window".into()),
            Some(block) if block.start < range.start || block.end > range.end => {
                rows.push(ReadRow::Imports(segment(&file, lines, block)));
                notes.push("the imports block is a lexical scan (leading import-shaped lines), not resolver output".into());
            }
            Some(_) => {}
        }
    }
    if range.start >= 1 && range.end >= range.start {
        for start in (range.start..=range.end).step_by(32) {
            rows.push(ReadRow::Lines(segment(
                &file,
                lines,
                ReadRange {
                    start,
                    end: start.saturating_add(31).min(range.end),
                },
            )));
        }
    }
    rows
}

fn invalid_selection(input: &ReadInput) -> Option<WorkspaceError> {
    if input.symbol.is_some() && input.lines.is_some() {
        return Some(WorkspaceError::new(
            ErrorKind::PatternInvalid,
            "read: symbol and lines are mutually exclusive",
        ));
    }
    input
        .lines
        .filter(|range| range.start == 0 || range.end == 0)
        .map(|_| {
            WorkspaceError::new(
                ErrorKind::PatternInvalid,
                "read: line bounds must be positive",
            )
        })
}

fn segment(file: &str, lines: &[&str], range: ReadRange) -> ReadSegment {
    ReadSegment {
        file: file.into(),
        start: range.start,
        end: range.end,
        text: lines[range.start - 1..range.end].join("\n"),
    }
}

fn import_block(lines: &[&str]) -> Option<ReadRange> {
    static IMPORT: OnceLock<Regex> = OnceLock::new();
    let pattern = IMPORT.get_or_init(|| Regex::new(r#"^\s*(?:import[\s(\"'{]|export\s+.*\sfrom\s|from\s+\S+\s+import[\s(]|use\s+[\w:]+.*;|require\s*\(|#include[\s<\"]|using\s+[\w.]+\s*;)"#).expect("static import pattern"));
    let mut found = lines
        .iter()
        .take(200)
        .enumerate()
        .filter(|(_, line)| pattern.is_match(line))
        .map(|(i, _)| i + 1);
    let start = found.next()?;
    Some(ReadRange {
        start,
        end: found.next_back().unwrap_or(start),
    })
}

fn failure_notes(error: WorkspaceError, notes: Vec<String>) -> WorkspaceResponse<ReadRow> {
    WorkspaceResponse::Failure {
        version: EnvelopeVersion::LATEST,
        error,
        notes: (!notes.is_empty()).then_some(notes),
    }
}

#[cfg(test)]
#[path = "read.tests.rs"]
mod tests;
