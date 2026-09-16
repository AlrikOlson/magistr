//! The journal model's state.

use std::collections::HashSet;

use crate::workbench::{JournalEntry, RunRow};

/// What the journal model shows: the write journal as the workbench last
/// answered it (the authoritative list of edits), which entry is
/// expanded to its diff, which entries the human approved, the last
/// run's rows (failing first), and the read-only viewer pane while one
/// is open.
///
/// Pure state: the journal arrives as
/// [`WorkbenchEvent::Journal`](super::WorkbenchEvent::Journal) and a
/// refresh replaces the list whole — approvals and the expansion
/// survive for the ids that survive.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Changes {
    entries: Vec<JournalEntry>,
    expanded: Option<String>,
    approved: HashSet<String>,
    ran: Option<String>,
    runs: Vec<RunRow>,
    viewer: Option<(String, Vec<String>)>,
}

impl Changes {
    /// The journal answered: the list is replaced whole; the expansion
    /// and the approvals stay for the ids still journaled.
    pub fn set_entries(&mut self, entries: Vec<JournalEntry>) {
        self.entries = entries;
        let known = |id: &String| self.entries.iter().any(|entry| entry.id == *id);
        if self.expanded.as_ref().is_some_and(|id| !known(id)) {
            self.expanded = None;
        }
        self.approved.retain(known);
    }

    /// The journaled edits, oldest first.
    #[must_use]
    pub fn entries(&self) -> &[JournalEntry] {
        &self.entries
    }

    /// The journal rewound `id`: its entry leaves the list.
    pub fn remove(&mut self, id: &str) {
        self.entries.retain(|entry| entry.id != id);
        if self.expanded.as_deref() == Some(id) {
            self.expanded = None;
        }
        self.approved.remove(id);
    }

    /// A click on the entry at `index`: its diff expands, or folds when
    /// it already shows — one entry expands at a time.
    pub fn toggle(&mut self, index: usize) {
        let Some(entry) = self.entries.get(index) else {
            return;
        };
        self.expanded = if self.expanded.as_deref() == Some(entry.id.as_str()) {
            None
        } else {
            Some(entry.id.clone())
        };
    }

    /// Whether the entry's diff shows.
    #[must_use]
    pub fn is_expanded(&self, id: &str) -> bool {
        self.expanded.as_deref() == Some(id)
    }

    /// \[Approve\] on the entry at `index`: marked reviewed, purely in
    /// the cockpit.
    pub fn approve(&mut self, index: usize) {
        if let Some(entry) = self.entries.get(index) {
            self.approved.insert(entry.id.clone());
        }
    }

    /// Whether the human approved the entry.
    #[must_use]
    pub fn is_approved(&self, id: &str) -> bool {
        self.approved.contains(id)
    }

    /// A run finished: its rows land, failing rows first (their order
    /// otherwise kept), replacing the previous run's.
    pub fn set_runs(&mut self, command: &str, mut rows: Vec<RunRow>) {
        rows.sort_by_key(|row| row.passed);
        self.ran = Some(command.to_owned());
        self.runs = rows;
    }

    /// The last run — its command and its rows, failing first — once
    /// one finished.
    #[must_use]
    pub fn runs(&self) -> Option<(&str, &[RunRow])> {
        self.ran
            .as_deref()
            .map(|command| (command, self.runs.as_slice()))
    }

    /// Open the read-only viewer pane on `title` over `lines`.
    pub fn open_viewer(&mut self, title: &str, lines: Vec<String>) {
        self.viewer = Some((title.to_owned(), lines));
    }

    /// The viewer pane's title and lines, while one is open.
    #[must_use]
    pub fn viewer(&self) -> Option<(&str, &[String])> {
        self.viewer
            .as_ref()
            .map(|(title, lines)| (title.as_str(), lines.as_slice()))
    }

    /// Close the viewer pane; whether one was open.
    pub fn close_viewer(&mut self) -> bool {
        self.viewer.take().is_some()
    }

    /// The row the tab always pins: the viewer pane's own title while
    /// one is open, else what the journal holds. A scrolled list still
    /// says how much of it there is.
    #[must_use]
    pub fn heading(&self) -> String {
        if let Some((title, _)) = &self.viewer {
            return title.clone();
        }
        if self.entries.is_empty() {
            return "changes · nothing journaled yet".to_owned();
        }
        let added: u32 = self.entries.iter().map(|entry| entry.added).sum();
        let removed: u32 = self.entries.iter().map(|entry| entry.removed).sum();
        format!(
            "changes · {} files · +{added} −{removed}",
            self.entries.len()
        )
    }

    /// The last run's verdict, once one has landed — failures counted
    /// out, because a failure is what brings a user back to the tab.
    /// One row whatever the run answered: the result rows themselves
    /// stay in the scrolling body, where they can be windowed.
    ///
    /// Silent while the viewer pane is open: the pane covers the
    /// journal the run belongs to, and it wears its own title.
    #[must_use]
    pub fn run_summary(&self) -> Option<String> {
        if self.viewer.is_some() {
            return None;
        }
        let command = self.ran.as_deref()?;
        let total = self.runs.len();
        let failed = self.runs.iter().filter(|row| !row.passed).count();
        Some(if total == 0 {
            format!("run {command} · no results")
        } else if failed == 0 {
            format!("run {command} · {total} passed")
        } else {
            format!("run {command} · {failed} failed of {total}")
        })
    }

    /// How many rows stay above the scroll — the count the view's
    /// `changes_rows` pins, so the window the keys and the wheel move
    /// is the window painted. Bounded at two by construction: the
    /// heading, and the run verdict once a run has landed.
    #[must_use]
    pub fn pinned_rows(&self) -> usize {
        // Derived from the strip itself rather than counted alongside
        // it, so the count and the row can never disagree — the view
        // paints a strip exactly when `run_summary` answers one.
        1 + usize::from(self.run_summary().is_some())
    }

    /// How many rows the tab's body has — the count the view's
    /// `changes_rows` produces, so the window the keys and the wheel
    /// move is the window painted.
    #[must_use]
    pub fn body_rows(&self) -> usize {
        if let Some((_, lines)) = &self.viewer {
            // The pane's title is pinned; only its lines scroll.
            return lines.len();
        }
        if self.is_empty() {
            // Nothing to scroll: the pinned heading is the empty state,
            // and a note under it would only say so a second time.
            return 0;
        }
        let entries = self.entries.len();
        // The run's heading is the pinned verdict strip; its result
        // rows scroll with everything else.
        entries + self.runs.len()
    }

    /// Whether the tab has nothing to show yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.ran.is_none() && self.viewer.is_none()
    }
}

#[cfg(test)]
#[path = "changes.tests.rs"]
mod tests;
