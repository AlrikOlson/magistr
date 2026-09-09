//! Inline markdown on one line: markers stripped, spans over what stays.

use super::span::Span;
use super::span_style::SpanStyle;

/// Parse one line's inline markdown: `**strong**`/`__strong__`,
/// `*emphasis*`/`_emphasis_`, `` `code` `` and `[label](url)`. Returns
/// the display text with the markers stripped and the spans styling it.
///
/// The pass is flat and left to right: a marker without its closer (or
/// with nothing between the pair) stays literal, and nothing nests — the
/// content of a matched pair is taken as it stands. Emphasis content
/// never starts or ends with whitespace (`2 * 3 * 4` stays literal), an
/// underscore run must stand at word boundaries (`a_variable_name` stays
/// literal), and a code span's content is never re-read for markers.
#[must_use]
pub fn parse_inline(text: &str) -> (String, Vec<Span>) {
    let mut display = String::with_capacity(text.len());
    let mut spans = Vec::new();
    let mut rest = text;
    let mut prev: Option<char> = None;
    while !rest.is_empty() {
        if let Some(token) = token(rest, prev) {
            let start = display.len();
            display.push_str(token.content);
            spans.push(Span::new(start, display.len(), token.style));
            rest = &rest[token.consumed..];
            prev = token.content.chars().next_back();
            continue;
        }
        let c = rest.chars().next().expect("rest is not empty");
        display.push(c);
        rest = &rest[c.len_utf8()..];
        prev = Some(c);
    }
    (display, spans)
}

/// A matched marker pair at the head of `rest`.
struct Token<'a> {
    /// Bytes of `rest` the pair and its content take.
    consumed: usize,
    /// What shows: the content between the markers.
    content: &'a str,
    style: SpanStyle,
}

fn token(rest: &str, prev: Option<char>) -> Option<Token<'_>> {
    let boundary = prev.is_none_or(|c| !c.is_alphanumeric());
    for (marker, style, needs_boundary) in [
        ("**", SpanStyle::Bold, false),
        ("__", SpanStyle::Bold, true),
    ] {
        if (!needs_boundary || boundary)
            && let Some(token) = emphasis(rest, marker, style, needs_boundary)
        {
            return Some(token);
        }
    }
    if let Some(token) = code(rest) {
        return Some(token);
    }
    for (marker, style, needs_boundary) in [
        ("*", SpanStyle::Italic, false),
        ("_", SpanStyle::Italic, true),
    ] {
        if (!needs_boundary || boundary)
            && let Some(token) = emphasis(rest, marker, style, needs_boundary)
        {
            return Some(token);
        }
    }
    link(rest)
}

/// An emphasis pair at the head of `rest`: content non-empty, no
/// whitespace against either marker; an underscore closer must also end
/// at a word boundary.
fn emphasis<'a>(
    rest: &'a str,
    marker: &str,
    style: SpanStyle,
    needs_boundary: bool,
) -> Option<Token<'a>> {
    let after = rest.strip_prefix(marker)?;
    let close = after.find(marker)?;
    let content = &after[..close];
    if content.is_empty()
        || content.starts_with(char::is_whitespace)
        || content.ends_with(char::is_whitespace)
    {
        return None;
    }
    let tail = &after[close + marker.len()..];
    if needs_boundary && tail.chars().next().is_some_and(char::is_alphanumeric) {
        return None;
    }
    Some(Token {
        consumed: marker.len() + close + marker.len(),
        content,
        style,
    })
}

/// A backtick pair at the head of `rest`; the content shows verbatim.
fn code(rest: &str) -> Option<Token<'_>> {
    let after = rest.strip_prefix('`')?;
    let close = after.find('`')?;
    if close == 0 {
        return None;
    }
    Some(Token {
        consumed: close + 2,
        content: &after[..close],
        style: SpanStyle::Code,
    })
}

/// `[label](url)` at the head of `rest`.
fn link(rest: &str) -> Option<Token<'_>> {
    let after = rest.strip_prefix('[')?;
    let label_end = after.find(']')?;
    let label = &after[..label_end];
    if label.is_empty() {
        return None;
    }
    let after_label = &after[label_end + 1..];
    let target = after_label.strip_prefix('(')?;
    let url_end = target.find(')')?;
    Some(Token {
        consumed: 1 + label_end + 2 + url_end + 1,
        content: label,
        style: SpanStyle::Link(target[..url_end].to_owned()),
    })
}

#[cfg(test)]
#[path = "inline.tests.rs"]
mod tests;
