//! Display width of text in terminal columns.
//!
//! Width is measured per grapheme cluster, not per `char`: a fully-qualified
//! emoji ZWJ sequence such as a family is one cluster two columns wide,
//! and a base letter with a combining mark is one column.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Number of terminal columns `text` occupies.
///
/// Control characters and lone combining marks are zero columns wide (a
/// control never reaches the byte stream as a cell); the result saturates
/// at `u16::MAX`.
#[must_use]
pub fn width(text: &str) -> u16 {
    if text.is_ascii() {
        // Every ASCII grapheme is one byte and one column, bar controls —
        // the same answer the segmenting path gives, without segmenting.
        let printable = text.bytes().filter(|b| !b.is_ascii_control()).count();
        return u16::try_from(printable).unwrap_or(u16::MAX);
    }
    text.graphemes(true)
        .map(grapheme_width)
        .fold(0u16, u16::saturating_add)
}

fn grapheme_width(grapheme: &str) -> u16 {
    if grapheme.chars().next().is_some_and(char::is_control) {
        return 0;
    }
    u16::try_from(UnicodeWidthStr::width(grapheme)).unwrap_or(u16::MAX)
}

#[cfg(test)]
#[path = "width.tests.rs"]
mod tests;
