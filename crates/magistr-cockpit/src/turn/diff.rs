//! The edit card's diff body, from the ACP diff's old and new text.

use similar::TextDiff;

/// Lines of context around each hunk — enough to place a change without
/// drowning the card in unchanged file.
const CONTEXT: usize = 2;

/// The unified diff body of an edit: `@@` hunk headers, `-` removed,
/// `+` added, ` ` context — one line each, no `---`/`+++` file header
/// (the card's head row already names the path). A new file (empty
/// `old`) is every line added, without a hunk header. Trailing newlines
/// are trimmed; the empty-to-empty edit is an empty body.
#[must_use]
pub fn body(old: &str, new: &str) -> String {
    if old.is_empty() {
        return new
            .lines()
            .map(|line| format!("+{line}"))
            .collect::<Vec<_>>()
            .join("\n");
    }
    let diff = TextDiff::from_lines(old, new);
    let mut out = diff.unified_diff().context_radius(CONTEXT).to_string();
    out.truncate(out.trim_end_matches('\n').len());
    out
}

#[cfg(test)]
#[path = "diff.tests.rs"]
mod tests;
