use qs_gpu::color::Srgba;

use crate::component::Component;
use crate::element::Element;
use crate::lexicon::Lexicon;
use crate::voice::Voice;

/// A line of text with the runs a query matched set bold in the accent — the volume's
/// snippet under a search result: mono ink with its `<b style="color:var(--mauve)">`
/// inside the sentence. a match is never underlined prose; the marked run is the
/// component's own register.
///
/// It wraps the way an inline box does: every word is its own run (its trailing space
/// attached, so a line end hangs the space exactly as the browser does), a word with a
/// mark inside it keeps its runs together, and the line breaks only between words.
pub struct MatchLine;

impl MatchLine {
    /// `text` in `voice` and `ink`, with the byte ranges in `marks`
    /// set bold in the accent. Marks are taken in order and clipped to
    /// the text; an empty list is a plain wrapping line.
    #[must_use]
    pub fn el(
        text: &str,
        marks: &[(usize, usize)],
        voice: Voice,
        ink: Srgba,
        lex: &Lexicon,
    ) -> Element {
        let bold = Voice {
            weight: 700,
            ..voice
        };
        let mut line = Element::row().wrap();
        for (at, word) in words(text) {
            let mut runs = Element::row().no_shrink();
            let end = at + word.len();
            let mut cursor = at;
            for &(s, e) in marks {
                let s = s.max(cursor).min(end);
                let e = e.min(end);
                if s >= e {
                    continue;
                }
                if s > cursor {
                    runs = runs.child(
                        Element::text(voice, ink, &text[cursor..s])
                            .claim(Component::SearchRow)
                            .nowrap(),
                    );
                }
                runs = runs.child(
                    Element::text(bold, lex.mauve, &text[s..e])
                        .claim(Component::SearchRow)
                        .nowrap(),
                );
                cursor = e;
            }
            if cursor < end {
                runs = runs.child(
                    Element::text(voice, ink, &text[cursor..end])
                        .claim(Component::SearchRow)
                        .nowrap(),
                );
            }
            line = line.child(runs);
        }
        line
    }
}

/// Each word of `text` with its byte offset, the single space that
/// follows it kept on the word — the separator the volume's copy uses.
fn words(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut at = 0;
    for piece in text.split_inclusive(' ') {
        if !piece.is_empty() {
            out.push((at, piece));
        }
        at += piece.len();
    }
    out
}

#[cfg(test)]
#[path = "match_line.tests.rs"]
mod tests;
