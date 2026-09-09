//! GitHub Actions annotation output.
//!
//! Parses compiler/linter output for `file:line` patterns and emits
//! GitHub Actions workflow commands (`::error file=...,line=...::message`).
//! These appear as inline annotations in PR diffs.
//!
//! Reference: <https://docs.github.com/en/actions/writing-workflows/choosing-what-your-workflow-does/workflow-commands-for-github-actions#setting-an-error-message>

use std::fmt::Write;

use crate::utils::gate::GateRun;
use crate::validate::ProjectValidation;

/// A single parsed annotation from compiler/linter output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Annotation {
    /// Severity level.
    pub level: AnnotationLevel,
    /// Relative file path.
    pub file: String,
    /// Line number (1-based).
    pub line: u32,
    /// Optional column number (1-based).
    pub col: Option<u32>,
    /// The diagnostic message.
    pub message: String,
    /// Which gate produced this annotation.
    pub title: String,
}

/// Severity level for GitHub Actions annotations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnnotationLevel {
    Error,
    Warning,
    Notice,
}

impl AnnotationLevel {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Notice => "notice",
        }
    }
}

impl Annotation {
    /// Format as a GitHub Actions workflow command.
    ///
    /// Example: `::error file=src/main.rs,line=10,col=5,title=lint::unused variable`
    #[must_use]
    pub fn to_workflow_command(&self) -> String {
        let mut cmd = format!(
            "::{} file={},line={}",
            self.level.as_str(),
            escape_property(&self.file),
            self.line
        );
        if let Some(col) = self.col {
            write!(cmd, ",col={col}").expect("write to String");
        }
        write!(
            cmd,
            ",title={}::{}",
            escape_property(&self.title),
            escape_data(&self.message)
        )
        .expect("write to String");
        cmd
    }
}

fn escape_data(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

fn escape_property(value: &str) -> String {
    escape_data(value).replace(':', "%3A").replace(',', "%2C")
}

/// Parse annotations from a gate result's output.
///
/// Recognizes common patterns:
/// - Rust: `error[E0308]: ... --> src/main.rs:10:5`
/// - Clippy: `warning: ... --> src/main.rs:10:5`
/// - TypeScript: `src/main.ts(10,5): error TS2322: ...`
/// - ESLint/generic: `src/main.ts:10:5: error ...`
/// - Python: `src/main.py:10: error: ...`
/// - Go: `src/main.go:10:5: ...`
#[must_use]
pub(crate) fn parse_annotations(gate: &GateRun) -> Vec<Annotation> {
    let title = &gate.name;
    let mut annotations = Vec::new();

    for line in gate.output.lines() {
        if let Some(ann) = try_parse_rust_arrow(line, title) {
            annotations.push(ann);
        } else if let Some(ann) = try_parse_ts_parens(line, title) {
            annotations.push(ann);
        } else if let Some(ann) = try_parse_colon_separated(line, title) {
            annotations.push(ann);
        }
    }

    annotations
}

/// Emit all annotations for a validation result as GitHub Actions workflow commands.
///
/// Only emits annotations for failed gates. Returns the full output string.
#[must_use]
pub(crate) fn format_annotations(result: &ProjectValidation) -> String {
    let mut output = String::new();

    for gate_result in &result.results {
        if gate_result.passed() {
            continue;
        }

        let annotations = parse_annotations(gate_result);

        if annotations.is_empty() {
            // No parseable file:line patterns — emit a general error annotation.
            writeln!(
                output,
                "::error title={}::{}",
                escape_property(&gate_result.name),
                escape_data(&format!(
                    "{} gate failed: {}",
                    gate_result.name, gate_result.command
                )),
            )
            .expect("write to String");
        } else {
            for ann in &annotations {
                writeln!(output, "{}", ann.to_workflow_command()).expect("write to String");
            }
        }
    }

    output
}

// ── Parsers ────────────────────────────────────────────────────────────────

/// Parse Rust-style `error[E0308]: msg` followed by ` --> file:line:col`.
///
/// Looks for lines matching ` --> path:line:col` and uses the preceding
/// error/warning line for the message.
fn try_parse_rust_arrow(line: &str, title: &str) -> Option<Annotation> {
    let trimmed = line.trim();

    // Match ` --> file:line:col`
    let arrow_prefix = "--> ";
    if !trimmed.starts_with(arrow_prefix) {
        return None;
    }

    let location = &trimmed[arrow_prefix.len()..];
    let parts: Vec<&str> = location.splitn(3, ':').collect();
    if parts.len() < 2 {
        return None;
    }

    let file = parts[0].to_string();
    let line_num: u32 = parts.get(1)?.parse().ok()?;
    let col: Option<u32> = parts.get(2).and_then(|s| s.parse().ok());

    Some(Annotation {
        level: AnnotationLevel::Error,
        file,
        line: line_num,
        col,
        message: "See compiler output for details".to_string(),
        title: title.to_string(),
    })
}

/// Parse TypeScript-style `file(line,col): error TS1234: message`.
fn try_parse_ts_parens(line: &str, title: &str) -> Option<Annotation> {
    let trimmed = line.trim();

    // Find the `(line,col):` pattern
    let paren_start = trimmed.find('(')?;
    let paren_end = trimmed[paren_start..].find(')')? + paren_start;

    let file = &trimmed[..paren_start];
    // Reject if file is empty or contains spaces (not a file path)
    if file.is_empty() || file.contains(' ') {
        return None;
    }

    let coords = &trimmed[paren_start + 1..paren_end];
    let coord_parts: Vec<&str> = coords.split(',').collect();
    let line_num: u32 = coord_parts.first()?.parse().ok()?;
    let col: Option<u32> = coord_parts.get(1).and_then(|s| s.parse().ok());

    // After `):`
    let after_paren = &trimmed[paren_end + 1..];
    let after_paren = after_paren.strip_prefix(':')?;
    let after_paren = after_paren.trim();

    let level = if after_paren.starts_with("error") {
        AnnotationLevel::Error
    } else if after_paren.starts_with("warning") {
        AnnotationLevel::Warning
    } else {
        AnnotationLevel::Notice
    };

    Some(Annotation {
        level,
        file: file.to_string(),
        line: line_num,
        col,
        message: after_paren.to_string(),
        title: title.to_string(),
    })
}

/// Parse colon-separated `file:line:col: level: message` (Go, ESLint, Python, etc.).
fn try_parse_colon_separated(line: &str, title: &str) -> Option<Annotation> {
    let trimmed = line.trim();

    // Split into at least `file:line:...`
    let parts: Vec<&str> = trimmed.splitn(4, ':').collect();
    if parts.len() < 3 {
        return None;
    }

    let file = parts[0];
    // Basic validation: file should look like a path (has a dot or slash)
    if !file.contains('.') && !file.contains('/') {
        return None;
    }
    // Reject if file contains spaces (likely not a path)
    if file.contains(' ') {
        return None;
    }

    let line_num: u32 = parts[1].trim().parse().ok()?;

    // parts[2] might be a column number or a level/message
    let (col, rest) = if let Ok(col_num) = parts[2].trim().parse::<u32>() {
        // file:line:col:rest
        let rest = parts.get(3).unwrap_or(&"").trim();
        (Some(col_num), rest.to_string())
    } else {
        // file:line:rest (no column)
        let rest = parts[2..].join(":");
        (None, rest.trim().to_string())
    };

    let level = if rest.starts_with("error") || rest.starts_with("E") {
        AnnotationLevel::Error
    } else if rest.starts_with("warning") || rest.starts_with("W") {
        AnnotationLevel::Warning
    } else {
        AnnotationLevel::Error // default to error for failed gates
    };

    let message = if rest.is_empty() {
        "See output for details".to_string()
    } else {
        rest
    };

    Some(Annotation {
        level,
        file: file.to_string(),
        line: line_num,
        col,
        message,
        title: title.to_string(),
    })
}

#[cfg(test)]
#[path = "annotations.tests.rs"]
mod tests;
