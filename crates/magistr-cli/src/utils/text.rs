//! Text truncation utilities.
//!
//! Canonical implementations replacing duplicates across handler.rs, compare.rs,
//! sidebar.rs, session_browser.rs, and resume.rs.

/// Truncate a string to `max` chars, appending "…" if truncated.
///
/// Operates on the first line only — multi-line input is reduced to its first line.
/// Returns the original string unchanged if it fits within `max`.
#[must_use]
pub(crate) fn truncate(s: &str, max: usize) -> String {
    let first_line = s.lines().next().unwrap_or(s);
    if first_line.chars().count() <= max {
        first_line.to_string()
    } else if max == 0 {
        String::new()
    } else {
        // Reserve 1 char for the ellipsis
        let end = char_boundary(first_line, max.saturating_sub(1));
        format!("{}…", &first_line[..end])
    }
}

/// Find the byte offset after the first `max_chars` characters, on a valid char boundary.
///
/// Returns the byte offset right after char at position `max_chars - 1`,
/// or `s.len()` if the string has fewer than `max_chars` characters.
pub(crate) fn char_boundary(s: &str, max_chars: usize) -> usize {
    s.char_indices()
        .take(max_chars)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "text.tests.rs"]
mod tests;
