//! A unified diff body, read as hunks a viewer can draw.

use similar::{ChangeTag, TextDiff};

use super::diff_kind::diff_kind;
use super::hunk_row::HunkRow;
use super::hunk_span::HunkSpan;
use super::row_kind::RowKind;

/// One hunk of a unified diff: where it starts on each side and the
/// lines it holds.
///
/// The input is the **unified body** both of the cockpit's producers
/// already write — [`diff::body`](super::diff::body) for an ACP edit
/// and the workbench for a journal entry — rather than a pair of file
/// texts, so there is one representation of a change in the cockpit and
/// the viewer draws exactly the lines the edit card and the Changes tab
/// draw. What it adds is the two things a `String` cannot carry: which
/// line of which file each row is, and which *part* of a replaced line
/// actually moved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    /// The first line number this hunk covers in the old file.
    pub old_start: u32,
    /// The first line number this hunk covers in the new file.
    pub new_start: u32,
    /// Its lines, in the order the diff writes them.
    pub rows: Vec<HunkRow>,
}

impl Hunk {
    /// Read `body` — a unified diff with no `---`/`+++` file header —
    /// as hunks, numbering every line from the `@@` headers and marking
    /// the runs a word diff finds changed.
    ///
    /// A body with no `@@` header at all is one hunk starting at line 1
    /// on both sides: that is what a new file's diff looks like
    /// ([`diff::body`](super::diff::body) writes every line `+` with no
    /// header), and refusing it would leave the one case where the
    /// numbers are least in doubt undrawable.
    #[must_use]
    pub fn parse(body: &str) -> Vec<Self> {
        let mut hunks: Vec<Self> = Vec::new();
        for line in body.lines() {
            if let Some((old_start, new_start)) = Self::header(line) {
                hunks.push(Self {
                    old_start,
                    new_start,
                    rows: Vec::new(),
                });
                continue;
            }
            let hunk = match hunks.last_mut() {
                Some(hunk) => hunk,
                None => {
                    hunks.push(Self {
                        old_start: 1,
                        new_start: 1,
                        rows: Vec::new(),
                    });
                    hunks.last_mut().expect("just pushed")
                }
            };
            hunk.push(line);
        }
        for hunk in &mut hunks {
            hunk.mark_words();
        }
        hunks
    }

    /// Columns the line-number gutter needs for `hunks` — the widest
    /// number any of them carries, and at least one column so an empty
    /// diff still has a gutter to blank.
    #[must_use]
    pub fn gutter_width(hunks: &[Self]) -> usize {
        let widest = hunks
            .iter()
            .flat_map(|hunk| hunk.rows.iter())
            .flat_map(|row| [row.old_no, row.new_no])
            .flatten()
            .max()
            .unwrap_or(1);
        widest.to_string().chars().count()
    }

    /// The hunk's rows paired for a two-column reading: the old side
    /// and the new side of each row of the view.
    ///
    /// A context line is on both sides — it is the same line, which is
    /// the whole reason a side-by-side layout is readable. A run of
    /// removals followed by a run of additions is paired position by
    /// position, so a replaced line sits beside the line that replaced
    /// it and the word-level runs line up; whichever run is longer
    /// spills into rows with nothing opposite them.
    #[must_use]
    pub fn side_by_side(&self) -> Vec<(Option<&HunkRow>, Option<&HunkRow>)> {
        let mut pairs = Vec::new();
        let mut index = 0;
        while index < self.rows.len() {
            let row = &self.rows[index];
            if row.kind != RowKind::DiffRemove && row.kind != RowKind::DiffAdd {
                pairs.push((Some(row), Some(row)));
                index += 1;
                continue;
            }
            let removed = Self::run(&self.rows[index..], RowKind::DiffRemove);
            let added = Self::run(&self.rows[index + removed..], RowKind::DiffAdd);
            for offset in 0..removed.max(added) {
                pairs.push((
                    (offset < removed).then(|| &self.rows[index + offset]),
                    (offset < added).then(|| &self.rows[index + removed + offset]),
                ));
            }
            index += removed + added;
        }
        pairs
    }

    /// How many rows from the front of `rows` are `kind`.
    fn run(rows: &[HunkRow], kind: RowKind) -> usize {
        rows.iter().take_while(|row| row.kind == kind).count()
    }

    /// `@@ -a,b +c,d @@` → the two starts. Both counts are optional in
    /// the format (`@@ -1 +1 @@` is legal for a one-line side) and are
    /// not needed: the rows themselves say how far each side runs.
    fn header(line: &str) -> Option<(u32, u32)> {
        let inner = line.strip_prefix("@@ ")?;
        let inner = inner.split(" @@").next()?;
        let mut sides = inner.split_whitespace();
        let old = Self::side(sides.next()?, '-')?;
        let new = Self::side(sides.next()?, '+')?;
        Some((old, new))
    }

    fn side(text: &str, sign: char) -> Option<u32> {
        text.strip_prefix(sign)?
            .split(',')
            .next()?
            .parse::<u32>()
            .ok()
    }

    /// Append one body line, numbering it from where the hunk stands.
    fn push(&mut self, line: &str) {
        let kind = diff_kind(line);
        // The *number* comes off the raw marker and the *kind* off the
        // one classifier: a conflict marker arrives as an added, a
        // removed or a context line, so `diff_kind` folding all three
        // into `Conflict` must not also decide which side advances.
        let marker = line.as_bytes().first().copied();
        let (old_no, new_no) = match marker {
            Some(b'-') => (Some(self.next_old()), None),
            Some(b'+') => (None, Some(self.next_new())),
            _ => (Some(self.next_old()), Some(self.next_new())),
        };
        let text = line
            .strip_prefix(['+', '-', ' '])
            .unwrap_or(line)
            .to_owned();
        self.rows.push(HunkRow {
            kind,
            old_no,
            new_no,
            spans: vec![HunkSpan::same(text)],
        });
    }

    fn next_old(&self) -> u32 {
        self.old_start
            + u32::try_from(self.rows.iter().filter(|r| r.old_no.is_some()).count())
                .unwrap_or(u32::MAX)
    }

    fn next_new(&self) -> u32 {
        self.new_start
            + u32::try_from(self.rows.iter().filter(|r| r.new_no.is_some()).count())
                .unwrap_or(u32::MAX)
    }

    /// Split every replaced line into the runs a word diff finds.
    ///
    /// Only a removal that has an addition opposite it is word-diffed:
    /// a line that was purely deleted or purely inserted has nothing to
    /// be different *from*, and marking the whole of it changed would
    /// say the same thing its `-`/`+` band already says while making
    /// every such row carry a redundant span.
    fn mark_words(&mut self) {
        let mut index = 0;
        while index < self.rows.len() {
            let removed = Self::run(&self.rows[index..], RowKind::DiffRemove);
            if removed == 0 {
                index += 1;
                continue;
            }
            let added = Self::run(&self.rows[index + removed..], RowKind::DiffAdd);
            for offset in 0..removed.min(added) {
                let old = self.rows[index + offset].text();
                let new = self.rows[index + removed + offset].text();
                self.rows[index + offset].spans = Self::runs(&old, &new, ChangeTag::Delete);
                self.rows[index + removed + offset].spans =
                    Self::runs(&old, &new, ChangeTag::Insert);
            }
            index += removed + added;
        }
    }

    /// One side of an intra-line diff between `old` and `new`: the
    /// side's own text, with the runs `tag` marks flagged changed and
    /// the runs both sides share flagged same. Adjacent runs of one
    /// flag are merged, so a viewer draws one band per change.
    ///
    /// The tokens are **characters**, merged back into runs, and not
    /// `similar`'s whitespace-delimited words: a word token carries its
    /// own punctuation, so `PaletteField)` becoming `PaletteField,` is
    /// one token replaced by another and the highlight covers a name
    /// that did not change. The merge is what keeps the result a word
    /// level rather than a character one — the runs a reader sees are
    /// whole insertions and deletions, and only the boundaries are
    /// decided per character.
    fn runs(old: &str, new: &str, tag: ChangeTag) -> Vec<HunkSpan> {
        let diff = TextDiff::from_chars(old, new);
        let mut spans: Vec<HunkSpan> = Vec::new();
        for change in diff.iter_all_changes() {
            let changed = match change.tag() {
                ChangeTag::Equal => false,
                other if other == tag => true,
                // The other side's run: not part of this side's text.
                _ => continue,
            };
            let value = change.value();
            match spans.last_mut() {
                Some(last) if last.changed == changed => last.text.push_str(value),
                _ => spans.push(HunkSpan {
                    text: value.to_owned(),
                    changed,
                }),
            }
        }
        if spans.is_empty() {
            spans.push(HunkSpan::same(String::new()));
        }
        spans
    }
}

#[cfg(test)]
#[path = "hunk.tests.rs"]
mod tests;
