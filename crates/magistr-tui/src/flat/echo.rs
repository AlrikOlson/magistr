//! Incremental echo of the live prompt, confined to one terminal row.

use magistr_cockpit::cockpit::Prompt;
use magistr_cockpit::render::width;
use unicode_segmentation::UnicodeSegmentation as _;

/// The live input row, never the settled transcript. The last terminal
/// column stays unused, so edits cannot wrap into another row. A multiline
/// prompt shows its current logical line with its line number.
#[derive(Default)]
pub(crate) struct Echo {
    shown: String,
    cursor: u16,
    columns: u16,
    active: bool,
}

impl Echo {
    /// End the live row before appending transcript, or returning the tty.
    /// Never move up: the settled scrollback belongs to the reader.
    pub(crate) fn finish(&mut self, out: &mut Vec<u8>) {
        if self.active {
            out.extend_from_slice(b"\r\n");
        }
        self.active = false;
        self.shown.clear();
        self.cursor = 0;
    }

    /// Record every model edit, including edits that cancel each other
    /// before the next frame. Only the draft may be overwritten.
    pub(crate) fn sync(&mut self, prompt: &Prompt, columns: u16, out: &mut Vec<u8>) {
        if self.active && self.columns != columns {
            // A resize may have reflowed the old row. Abandon its cursor
            // coordinates instead of trying to erase through scrollback.
            self.finish(out);
        }
        self.columns = columns;
        let (shown, cursor) = line(prompt, columns.saturating_sub(1));
        if !self.active {
            out.extend_from_slice(b"\r\x1b[K");
            write_suffix(out, &shown, 0);
            move_cursor(out, width(&shown), cursor);
            self.active = true;
        } else if shown == self.shown {
            move_cursor(out, self.cursor, cursor);
        } else {
            // Appending a combining mark or a ZWJ continues the existing
            // cluster. Reprinting its base would echo the base twice.
            let common = if shown.starts_with(&self.shown) {
                self.shown.len()
            } else {
                self.shown
                    .graphemes(true)
                    .zip(shown.graphemes(true))
                    .take_while(|(old, new)| old == new)
                    .map(|(old, _)| old.len())
                    .sum()
            };
            let at = width(&shown[..common]);
            move_cursor(out, self.cursor, at);
            write_suffix(out, &shown, common);
            let end = width(&shown);
            if self.shown.is_ascii() {
                let erased = width(&self.shown).saturating_sub(end);
                out.extend(std::iter::repeat_n(b' ', usize::from(erased)));
                move_cursor(out, end.saturating_add(erased), cursor);
            } else {
                // The old Unicode row may have occupied more columns
                // than its logical width on a nonjoining terminal.
                out.extend_from_slice(b"\x1b[K");
                move_cursor(out, end, cursor);
            }
        }
        self.shown = shown;
        self.cursor = cursor;
    }
}

/// Project the cursor's logical line, sanitizing controls before measuring.
/// A one-cell terminal has no writable draft cells; never trigger autowrap.
fn line(prompt: &Prompt, room: u16) -> (String, u16) {
    let row = prompt.text()[..prompt.cursor()].matches('\n').count() + 1;
    let label = if prompt.rows() > 1 {
        format!("input {row}/{}: ", prompt.rows())
    } else {
        "input: ".to_owned()
    };
    // Keep some editing room in narrow terminals by dropping the label.
    let label = if width(&label).saturating_add(2) <= room {
        label
    } else {
        String::new()
    };
    let available = room.saturating_sub(width(&label));
    let before = &prompt.text()[prompt.line_start()..prompt.cursor()];
    let after = prompt.text()[prompt.cursor()..]
        .split('\n')
        .next()
        .unwrap_or("");
    let under = after.graphemes(true).next().map(shown);
    let cursor_room = under
        .as_deref()
        .map_or(1, width_ceiling)
        .max(1)
        .min(available);
    let mut used = 0u16;
    let mut tail = Vec::new();
    for grapheme in before.graphemes(true).rev() {
        let cell = shown(grapheme);
        let cells = width_ceiling(&cell);
        if used.saturating_add(cells) > available.saturating_sub(cursor_room) {
            break;
        }
        used = used.saturating_add(cells);
        tail.push(cell);
    }
    let mut text = label;
    for cell in tail.iter().rev() {
        text.push_str(cell);
    }
    let cursor = width(&text);
    used = width_ceiling(&text);
    for grapheme in after.graphemes(true) {
        let cell = shown(grapheme);
        let cells = width_ceiling(&cell);
        if used.saturating_add(cells) > room {
            break;
        }
        used = used.saturating_add(cells);
        text.push_str(&cell);
    }
    (text, cursor)
}

/// Reserve enough room even on terminals that advance once per scalar
/// instead of joining an emoji cluster. Cursor positions remain logical.
fn width_ceiling(text: &str) -> u16 {
    text.graphemes(true).fold(0u16, |sum, grapheme| {
        let scalars = grapheme.chars().fold(0u16, |cells, ch| {
            let mut bytes = [0; 4];
            cells.saturating_add(width(ch.encode_utf8(&mut bytes)))
        });
        sum.saturating_add(width(grapheme).max(scalars))
    })
}

/// Write whole clusters, except when appending to the last existing one.
/// After Unicode, terminal cursor advance is not portable: reanchor on
/// this row and clear expanded remnants before writing the next suffix.
fn write_suffix(out: &mut Vec<u8>, text: &str, common: usize) {
    let mut column = 0u16;
    for (start, grapheme) in text.grapheme_indices(true) {
        column = column.saturating_add(width(grapheme));
        let end = start + grapheme.len();
        if end <= common {
            continue;
        }
        out.extend_from_slice(&text.as_bytes()[start.max(common)..end]);
        if !grapheme.is_ascii() {
            out.extend_from_slice(format!("\x1b[{}G\x1b[K", column.saturating_add(1)).as_bytes());
        }
    }
}

/// Terminal controls are literal, readable text; Unicode composition stays
/// intact. Prompt::window_rows targets cells and is not a safe wire encoder.
fn shown(grapheme: &str) -> String {
    let text: String = grapheme
        .chars()
        .map(|ch| match ch {
            '\t' => "\\t".to_owned(),
            ch if ch.is_ascii_control() => {
                format!(
                    "^{}",
                    if ch == '\u{7f}' {
                        '?'
                    } else {
                        char::from((ch as u8) + 64)
                    }
                )
            }
            ch if ch.is_control() => format!("\\u{{{:x}}}", u32::from(ch)),
            ch => ch.to_string(),
        })
        .collect();
    if !text.is_empty() && width(&text) == 0 {
        // A leading combining mark must not attach to the input label.
        format!("◌{text}")
    } else {
        text
    }
}

fn move_cursor(out: &mut Vec<u8>, from: u16, to: u16) {
    if to < from {
        out.extend(std::iter::repeat_n(b'\x08', usize::from(from - to)));
    } else if to > from {
        out.extend_from_slice(format!("\x1b[{}C", to - from).as_bytes());
    }
}

#[cfg(test)]
#[path = "echo.tests.rs"]
mod tests;
