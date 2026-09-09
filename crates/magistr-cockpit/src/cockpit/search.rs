//! ⌘⇧F search: one query over what the model already holds.

use super::model::Model;
use super::search_hit::{SearchGo, SearchHit};
use super::search_kind::SearchKind;
use super::search_results::SearchResults;
use super::search_scope::SearchScope;
use super::ship_moment::ShipMoment;

/// The ⌘⇧F overlay's state: the query being typed, which row is
/// chosen and which scope chip is picked. The *results* are never
/// stored — [`Search::results`] derives them from the model on every
/// read, so a hit is as live as the store it came from.
///
/// The palette jumps to places; search finds things. It reads what think-and-ship and the launcher already keep —
/// shipped chunks, trace steps, ship actions, recent sessions — and
/// never a second corpus. Raised only on a native surface, the Brief's
/// rule: an overlay needs an altitude to float over.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Search {
    query: String,
    chosen: usize,
    scope: SearchScope,
}

impl Search {
    /// Rows each kind is cut to under *everything* — the strongest few
    /// of each, so the list stays a glance; a scope chip is how more of
    /// one kind is read.
    pub const CUT_EACH: usize = 2;
    /// Rows a single scoped kind is cut to.
    pub const CUT_SCOPED: usize = 8;
    /// The snippet window before and after the match, in bytes, before
    /// the cut retreats to a clause or a word.
    const BEFORE: usize = 64;
    const AFTER: usize = 32;
    /// The cut's ellipsis.
    const ELLIPSIS: &'static str = "\u{2026}";

    /// What has been typed.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Which row is chosen, an index into [`SearchResults::rows`].
    #[must_use]
    pub const fn chosen(&self) -> usize {
        self.chosen
    }

    /// Which scope chip is picked.
    #[must_use]
    pub const fn scope(&self) -> SearchScope {
        self.scope
    }

    /// Type one character; the choice snaps back to the first row.
    pub(crate) fn type_char(&mut self, c: char) {
        self.query.push(c);
        self.chosen = 0;
    }

    /// Erase the last character; the choice snaps back likewise.
    pub(crate) fn backspace(&mut self) {
        self.query.pop();
        self.chosen = 0;
    }

    /// Walk the choice up.
    pub(crate) const fn up(&mut self) {
        self.chosen = self.chosen.saturating_sub(1);
    }

    /// Walk the choice down, clamped to the last of `len` rows.
    pub(crate) fn down(&mut self, len: usize) {
        self.chosen = (self.chosen + 1).min(len.saturating_sub(1));
    }

    /// Pick a scope; the choice snaps back to the first row.
    pub(crate) fn set_scope(&mut self, scope: SearchScope) {
        self.scope = scope;
        self.chosen = 0;
    }

    /// The chip to the right (⇥).
    pub(crate) fn next_scope(&mut self) {
        self.set_scope(self.scope.next());
    }

    /// The chip to the left (⇧⇥).
    pub(crate) fn prev_scope(&mut self) {
        self.set_scope(self.scope.prev());
    }

    /// What the model's open search finds — empty when the search is
    /// down or nothing has been typed.
    ///
    /// The query is words: a candidate hits when any word occurs in it
    /// (ASCII case-folded), and ranks by the whole phrase occurring,
    /// then by how many distinct words do; ties keep the corpus's own
    /// order. Kinds come in the book's order, each cut to its
    /// strongest few; the total counts every hit before the cut.
    #[must_use]
    pub fn results(model: &Model) -> SearchResults {
        let Some(search) = model.search() else {
            return SearchResults::default();
        };
        let needles = Needles::of(search.query());
        if needles.words.is_empty() {
            return SearchResults::default();
        }
        let cut = if search.scope == SearchScope::Everything {
            Self::CUT_EACH
        } else {
            Self::CUT_SCOPED
        };
        let mut results = SearchResults::default();
        for kind in SearchKind::ALL {
            if search.scope.kind().is_some_and(|only| only != kind) {
                continue;
            }
            let mut scored: Vec<(i32, usize, Candidate)> = candidates(model, kind)
                .into_iter()
                .enumerate()
                .filter_map(|(order, candidate)| {
                    needles
                        .score(&candidate.haystack())
                        .map(|score| (score, order, candidate))
                })
                .collect();
            results.total += scored.len();
            scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            results.rows.extend(
                scored
                    .into_iter()
                    .take(cut)
                    .map(|(_, _, candidate)| candidate.into_hit(&needles)),
            );
        }
        results
    }
}

/// One thing the corpus holds, before the query touched it.
struct Candidate {
    kind: SearchKind,
    title: Option<String>,
    /// The text the line is cut from.
    text: String,
    /// Whether the line is a window on `text` (a chapter, a step) or
    /// `text` whole (a receipt's, a session's identity).
    windowed: bool,
    /// What the line opens and closes with around the window.
    prefix: String,
    suffix: String,
    when: Option<String>,
    go: SearchGo,
}

impl Candidate {
    /// What the query is scored against: the title and the text.
    fn haystack(&self) -> String {
        match &self.title {
            Some(title) => format!("{title}\n{}", self.text),
            None => self.text.clone(),
        }
    }

    /// The hit: the line cut around the match with its marks.
    fn into_hit(self, needles: &Needles) -> SearchHit {
        let (body, mut marks) = if self.windowed {
            snippet(&self.text, needles)
        } else {
            (self.text.clone(), needles.marks(&self.text))
        };
        let shift = self.prefix.len();
        for mark in &mut marks {
            mark.0 += shift;
            mark.1 += shift;
        }
        SearchHit {
            kind: self.kind,
            title: self.title,
            line: format!("{}{body}{}", self.prefix, self.suffix),
            marks,
            when: self.when,
            go: self.go,
        }
    }
}

/// Everything of one kind the model holds, in the corpus's own order.
fn candidates(model: &Model, kind: SearchKind) -> Vec<Candidate> {
    match kind {
        SearchKind::Report => model
            .roadmap()
            .chapters()
            .into_iter()
            .map(|chunk| {
                let title = if chunk.title.is_empty() {
                    chunk.name.clone()
                } else {
                    chunk.title.clone()
                };
                let text = if chunk.description.is_empty() {
                    title.clone()
                } else {
                    chunk.description.clone()
                };
                Candidate {
                    kind,
                    title: Some(title),
                    text,
                    windowed: true,
                    prefix: String::new(),
                    suffix: String::new(),
                    when: chunk.updated_secs.and_then(day),
                    go: SearchGo::Chunk(chunk.id.clone()),
                }
            })
            .collect(),
        SearchKind::Trace => model
            .trace()
            .steps()
            .iter()
            .map(|step| {
                let text = if step.thought.is_empty() {
                    step.purpose.clone()
                } else {
                    step.thought.clone()
                };
                Candidate {
                    kind,
                    title: None,
                    text,
                    windowed: true,
                    prefix: format!("think:{} \u{b7} \u{201c}", step.number),
                    suffix: "\u{201d}".to_owned(),
                    when: None,
                    go: SearchGo::Step(step.number),
                }
            })
            .collect(),
        SearchKind::Receipt => model
            .trace()
            .moments()
            .iter()
            .filter_map(|moment| match moment {
                ShipMoment::Action {
                    kind: verb,
                    description,
                    id,
                    files,
                    ..
                } => {
                    let mut parts: Vec<String> = Vec::new();
                    if let Some(id) = id {
                        parts.push(format!("#{id}"));
                    }
                    if !verb.is_empty() {
                        parts.push(verb.clone());
                    }
                    if let Some(file) = files.first() {
                        parts.push(file.clone());
                    }
                    if !description.is_empty() {
                        parts.push(description.clone());
                    }
                    Some(Candidate {
                        kind,
                        title: None,
                        text: parts.join(" \u{b7} "),
                        windowed: false,
                        prefix: String::new(),
                        suffix: String::new(),
                        when: None,
                        go: SearchGo::Receipt,
                    })
                }
                ShipMoment::Check { .. } => None,
            })
            .collect(),
        SearchKind::Session => model
            .launcher()
            .map(|data| {
                data.sessions
                    .iter()
                    .enumerate()
                    .map(|(index, session)| Candidate {
                        kind,
                        title: None,
                        text: [
                            session.when.as_str(),
                            session.duration.as_str(),
                            session.preview.as_str(),
                        ]
                        .into_iter()
                        .filter(|part| !part.is_empty())
                        .collect::<Vec<_>>()
                        .join(" \u{b7} "),
                        windowed: false,
                        prefix: String::new(),
                        suffix: String::new(),
                        when: None,
                        go: SearchGo::Session(index),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// The query as the matcher reads it: its words, case-folded, and the
/// whole phrase when there is more than one.
struct Needles {
    words: Vec<String>,
    phrase: Option<String>,
}

impl Needles {
    fn of(query: &str) -> Self {
        let words: Vec<String> = query
            .split_whitespace()
            .map(str::to_ascii_lowercase)
            .collect();
        let phrase = (words.len() > 1).then(|| words.join(" "));
        Self { words, phrase }
    }

    /// How strongly `hay` answers the query: the phrase outranks any
    /// number of scattered words, and among those more distinct words
    /// rank higher. `None` when no word occurs at all.
    fn score(&self, hay: &str) -> Option<i32> {
        let hay = hay.to_ascii_lowercase();
        let words = self
            .words
            .iter()
            .filter(|w| hay.contains(w.as_str()))
            .count();
        if words == 0 {
            return None;
        }
        let phrase = self
            .phrase
            .as_ref()
            .is_some_and(|p| hay.contains(p.as_str()));
        Some(i32::from(phrase) * 8 + i32::try_from(words).unwrap_or(i32::MAX) * 2)
    }

    /// Every run of `text` the query matched — phrase and words
    /// together, overlaps merged — as byte ranges, in order.
    fn marks(&self, text: &str) -> Vec<(usize, usize)> {
        let hay = text.to_ascii_lowercase();
        let mut marks: Vec<(usize, usize)> = self
            .phrase
            .iter()
            .chain(self.words.iter())
            .flat_map(|needle| occurrences(&hay, needle))
            .collect();
        marks.sort_unstable();
        let mut merged: Vec<(usize, usize)> = Vec::new();
        for mark in marks {
            match merged.last_mut() {
                Some(last) if mark.0 <= last.1 => last.1 = last.1.max(mark.1),
                _ => merged.push(mark),
            }
        }
        merged
    }

    /// Where the window centres: the first phrase occurrence, else the
    /// earliest word.
    fn anchor(&self, text: &str) -> Option<(usize, usize)> {
        let hay = text.to_ascii_lowercase();
        if let Some(at) = self
            .phrase
            .as_ref()
            .and_then(|p| occurrences(&hay, p).into_iter().next())
        {
            return Some(at);
        }
        self.words
            .iter()
            .filter_map(|w| occurrences(&hay, w).into_iter().next())
            .min()
    }
}

/// Every non-overlapping occurrence of `needle` in `hay`.
fn occurrences(hay: &str, needle: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if needle.is_empty() {
        return out;
    }
    let mut at = 0;
    while let Some(found) = hay[at..].find(needle) {
        let start = at + found;
        out.push((start, start + needle.len()));
        at = start + needle.len();
    }
    out
}

/// The window on `text` around the query's anchor, cut to a clause
/// (`, ` `; ` `. ` `: `) when one falls inside the window and to a word
/// otherwise, with an ellipsis on every cut edge — and the marks, moved
/// into the window.
fn snippet(text: &str, needles: &Needles) -> (String, Vec<(usize, usize)>) {
    let anchor = needles.anchor(text).unwrap_or((0, 0));
    let mut start = anchor.0.saturating_sub(Search::BEFORE);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    if start > 0 {
        let head = &text[start..anchor.0];
        start += clause_after(head)
            .or_else(|| head.find(char::is_whitespace).map(|i| i + 1))
            .unwrap_or(0);
    }
    let mut end = (anchor.1 + Search::AFTER).min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    if end < text.len() {
        let tail = &text[anchor.1..end];
        end = anchor.1
            + clause_before(tail)
                .or_else(|| tail.rfind(char::is_whitespace))
                .unwrap_or(tail.len());
    }
    let window = &text[start..end];
    let trimmed = window.trim();
    let lead = start + (window.len() - window.trim_start().len());
    let cut_front = lead > 0;
    let cut_back = lead + trimmed.len() < text.len();
    let mut line = String::new();
    if cut_front {
        line.push_str(Search::ELLIPSIS);
    }
    let shift = line.len();
    line.push_str(trimmed);
    if cut_back {
        line.push_str(Search::ELLIPSIS);
    }
    let marks = needles
        .marks(text)
        .into_iter()
        .filter_map(|(s, e)| {
            let s = s.max(lead);
            let e = e.min(lead + trimmed.len());
            (s < e).then(|| (s - lead + shift, e - lead + shift))
        })
        .collect();
    (line, marks)
}

/// The byte just after the first clause boundary in `head` — a `,`
/// `;` `.` or `:` followed by whitespace — when there is one.
fn clause_after(head: &str) -> Option<usize> {
    head.char_indices()
        .find(|(i, c)| {
            is_clause_mark(*c) && head[i + c.len_utf8()..].starts_with(char::is_whitespace)
        })
        .map(|(i, c)| {
            let rest = &head[i + c.len_utf8()..];
            i + c.len_utf8() + (rest.len() - rest.trim_start().len())
        })
}

/// The byte of the last clause boundary's mark in `tail`, when there
/// is one.
fn clause_before(tail: &str) -> Option<usize> {
    tail.char_indices()
        .filter(|(i, c)| {
            is_clause_mark(*c) && tail[i + c.len_utf8()..].starts_with(char::is_whitespace)
        })
        .map(|(i, _)| i)
        .next_back()
}

const fn is_clause_mark(c: char) -> bool {
    matches!(c, ',' | ';' | '.' | ':')
}

/// A stamp in seconds since the epoch as the copy laws print a day —
/// `Aug 29`.
fn day(secs: i64) -> Option<String> {
    chrono::DateTime::<chrono::Utc>::from_timestamp(secs, 0)
        .map(|stamp| stamp.format("%b %-d").to_string())
}

#[cfg(test)]
#[path = "search.tests.rs"]
mod tests;
