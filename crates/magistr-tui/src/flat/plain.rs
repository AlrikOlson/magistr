//! Cockpit text with its glyphs taken out.

/// `text` with the cockpit's decorative glyphs removed and its two
/// separator dots turned into a hyphen, runs of spaces squeezed and the
/// ends trimmed.
///
/// Applied to the **cockpit's own** lines — events, notes, errors, the
/// banner — and never to the agent's prose, which is content and must
/// arrive as it was written. That split is why this can strip by
/// Unicode *range* rather than by a list of glyphs that would drift
/// from [`Icons`](magistr_cockpit::theme::Icons): the ranges below are where a
/// terminal UI's vocabulary lives (arrows, box drawing, blocks,
/// geometric shapes, miscellaneous symbols, dingbats) plus the private
/// use areas a Nerd Font puts its icons in — none of which a sentence
/// the cockpit writes about itself has any other use for.
#[must_use]
pub fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            // The two separator dots `detailed` joins with. Dropping
            // them would run `what` into `detail`.
            '\u{00b7}' | '\u{2022}' => out.push('-'),
            ch if is_decorative(ch) => {}
            ch => out.push(ch),
        }
    }
    squeeze(&out)
}

/// Whether `ch` belongs to one of the ranges a terminal UI draws its
/// chrome and its icons from.
fn is_decorative(ch: char) -> bool {
    matches!(ch,
        // Arrows.
        '\u{2190}'..='\u{21ff}'
        // Box drawing, block elements, geometric shapes, miscellaneous
        // symbols, dingbats — one contiguous run.
        | '\u{2500}'..='\u{27bf}'
        // Miscellaneous symbols and arrows.
        | '\u{2b00}'..='\u{2bff}'
        // Variation selectors and the emoji join.
        | '\u{fe00}'..='\u{fe0f}'
        | '\u{200d}'
        // Private use: where a Nerd Font keeps its icons.
        | '\u{e000}'..='\u{f8ff}'
        | '\u{f0000}'..='\u{ffffd}'
        | '\u{100000}'..='\u{10fffd}'
    )
}

/// Runs of whitespace collapsed to one space, both ends trimmed — what
/// a removed glyph leaves behind.
fn squeeze(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for ch in text.trim().chars() {
        if ch.is_whitespace() {
            space = true;
            continue;
        }
        if space && !out.is_empty() {
            out.push(' ');
        }
        space = false;
        out.push(ch);
    }
    out
}

#[cfg(test)]
#[path = "plain.tests.rs"]
mod tests;
