//! Syntax highlighting for one fenced block, one closed line at a time.

use std::fmt;
use std::sync::OnceLock;

use syntect::highlighting::{HighlightIterator, HighlightState, Highlighter, ThemeSet};
use syntect::parsing::{ParseState, ScopeStack, SyntaxSet};

use super::span::Span;
use super::span_style::SpanStyle;
use crate::render::Color;
use crate::theme::ThemeMode;

/// One fenced code block's highlighting, advanced a line at a time.
///
/// The state is syntect's own incremental pair — [`ParseState`] and
/// [`HighlightState`] — so a multi-line construct (a raw string, a block
/// comment) carries across lines. [`Highlight::line`] is called exactly
/// once per closed line; the still-open tail of a streaming block is
/// rendered plain until it closes. The syntax and theme sets are process
/// globals loaded from syntect's embedded defaults on first use.
#[derive(Clone)]
pub struct Highlight {
    parse: ParseState,
    state: HighlightState,
    mode: ThemeMode,
    lines: usize,
    lang: String,
}

impl Highlight {
    /// Highlighting for the block a ```` ```lang ```` line opens, in the
    /// theme `mode` calls for. An unknown or empty language highlights
    /// as plain text.
    #[must_use]
    pub fn for_fence(fence_line: &str, mode: ThemeMode) -> Self {
        let lang = fence_line
            .trim_start()
            .trim_start_matches('`')
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_owned();
        let syntaxes = syntaxes();
        let syntax = syntaxes
            .find_syntax_by_token(&lang)
            .unwrap_or_else(|| syntaxes.find_syntax_plain_text());
        Self {
            parse: ParseState::new(syntax),
            state: HighlightState::new(highlighter(mode), ScopeStack::new()),
            mode,
            lines: 0,
            lang,
        }
    }

    /// The spans for one closed code line, advancing the block's state
    /// past it. A line syntect cannot parse renders plain.
    pub fn line(&mut self, text: &str) -> Vec<Span> {
        self.lines += 1;
        // The default syntaxes are the newline flavour: they expect the
        // terminator on the line.
        let terminated = format!("{text}\n");
        let Ok(ops) = self.parse.parse_line(&terminated, syntaxes()) else {
            return Vec::new();
        };
        let highlighter = highlighter(self.mode);
        let mut spans = Vec::new();
        let mut at = 0;
        for (style, piece) in
            HighlightIterator::new(&mut self.state, &ops, &terminated, highlighter)
        {
            let end = (at + piece.len()).min(text.len());
            if at < end {
                let fg = style.foreground;
                spans.push(Span::new(
                    at,
                    end,
                    SpanStyle::Fg(Color::Rgb(fg.r, fg.g, fg.b)),
                ));
            }
            at += piece.len();
        }
        spans
    }
}

/// Two blocks compare equal when they would render the same rows again:
/// same language, same theme mode, same number of lines fed. The syntect
/// states themselves have no equality and derive from these.
impl PartialEq for Highlight {
    fn eq(&self, other: &Self) -> bool {
        self.lang == other.lang && self.mode == other.mode && self.lines == other.lines
    }
}

impl Eq for Highlight {}

impl fmt::Debug for Highlight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Highlight")
            .field("lang", &self.lang)
            .field("mode", &self.mode)
            .field("lines", &self.lines)
            .finish_non_exhaustive()
    }
}

/// The embedded default syntaxes (newline flavour), loaded once.
fn syntaxes() -> &'static SyntaxSet {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// The highlighter for `mode`, built once per mode from the embedded
/// default themes.
fn highlighter(mode: ThemeMode) -> &'static Highlighter<'static> {
    static THEMES: OnceLock<ThemeSet> = OnceLock::new();
    static DARK: OnceLock<Highlighter<'static>> = OnceLock::new();
    static LIGHT: OnceLock<Highlighter<'static>> = OnceLock::new();
    let themes = THEMES.get_or_init(ThemeSet::load_defaults);
    let (slot, name) = match mode {
        ThemeMode::Dark => (&DARK, "base16-eighties.dark"),
        ThemeMode::Light => (&LIGHT, "InspiredGitHub"),
    };
    slot.get_or_init(|| {
        Highlighter::new(
            themes
                .themes
                .get(name)
                .expect("syntect's default themes carry it"),
        )
    })
}

#[cfg(test)]
#[path = "highlight.tests.rs"]
mod tests;
