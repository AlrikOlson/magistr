use similar::{ChangeTag, TextDiff};

use crate::file_content::FileContent;

/// Unified diff of two file states in the retained dialect: `a/<path>` and
/// `b/<path>` headers, `/dev/null` for absence, a byte summary for binary
/// content, and the empty string when the states are identical.
pub(super) fn unified_diff(
    path: &str,
    before: Option<&FileContent>,
    after: Option<&FileContent>,
) -> String {
    if before == after {
        return String::new();
    }
    let from_name = before.map_or("/dev/null".to_owned(), |_| format!("a/{path}"));
    let to_name = after.map_or("/dev/null".to_owned(), |_| format!("b/{path}"));
    match (text(before), text(after)) {
        (Some(before), Some(after)) => TextDiff::from_lines(before, after)
            .unified_diff()
            .context_radius(3)
            .header(&from_name, &to_name)
            .to_string(),
        _ => format!(
            "Binary files {from_name} and {to_name} differ\nBefore: {}\nAfter: {}\n",
            describe(before),
            describe(after)
        ),
    }
}

/// Lines a diff would apply: inserted plus deleted lines, headers excluded.
pub(super) fn changed_lines(before: Option<&FileContent>, after: Option<&FileContent>) -> usize {
    if before == after {
        return 0;
    }
    match (text(before), text(after)) {
        (Some(before), Some(after)) => TextDiff::from_lines(before, after)
            .iter_all_changes()
            .filter(|change| change.tag() != ChangeTag::Equal)
            .count(),
        // A binary transition renders as one summary block: count it as one line.
        _ => 1,
    }
}

fn text(content: Option<&FileContent>) -> Option<&str> {
    match content {
        Some(FileContent::Text(text)) => Some(text.as_str()),
        Some(FileContent::Binary(_)) => None,
        None => Some(""),
    }
}

fn describe(content: Option<&FileContent>) -> String {
    content.map_or("absent".to_owned(), |content| {
        format!(
            "{} bytes, sha256 {}",
            content.as_bytes().len(),
            content.content_hash()
        )
    })
}

#[cfg(test)]
#[path = "diff.tests.rs"]
mod tests;
