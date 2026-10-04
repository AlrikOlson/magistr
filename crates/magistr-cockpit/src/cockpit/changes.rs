//! The Changes feature — the workbench's write journal as it stands,
//! and the one owner of how a journaled change is read, rewound,
//! approved and refused from the screen.

use std::collections::{HashMap, HashSet};

use super::cmd::Cmd;
use super::decision;
use super::model::Model;
use super::pane::Pane;
use super::selected::Selected;
use super::workbench_cmd::WorkbenchCmd;
use super::workbench_event::WorkbenchEvent;
use crate::workbench::{JournalEntry, RunRow};

/// What the journal model shows: the write journal as the workbench last
/// answered it (the authoritative list of applied edits — never the
/// diff a tool call carried, which is the agent's intent on the turn),
/// which entry is expanded to its diff, which entries the human
/// approved, which rewinds the workbench refused and why (a later
/// change touched the file, an op another session owns), the last run's
/// rows (failing first), and the read-only viewer pane while one is
/// open.
///
/// Pure state: the journal arrives as
/// [`WorkbenchEvent::Journal`] and a refresh replaces the list whole —
/// approvals, refusals and the expansion survive for the ids that
/// survive. Entries are anchored by the journal's own id and path
/// ([`JournalEntry::identity`]), never by a row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Changes {
    entries: Vec<JournalEntry>,
    expanded: Option<String>,
    approved: HashSet<String>,
    refused: HashMap<String, String>,
    ran: Option<String>,
    runs: Vec<RunRow>,
    viewer: Option<(String, Vec<String>)>,
}

impl Changes {
    /// The journal answered: the list is replaced whole; the expansion,
    /// the approvals and the refusals stay for the ids still journaled.
    pub fn set_entries(&mut self, entries: Vec<JournalEntry>) {
        self.entries = entries;
        self.retain_flags();
        self.refused
            .retain(|id, _| self.entries.iter().any(|entry| entry.id == *id));
    }

    /// Keep expansion and review attached to a file row, not its position.
    fn retain_flags(&mut self) {
        let known: HashSet<String> = self.entries.iter().map(JournalEntry::identity).collect();
        if self
            .expanded
            .as_ref()
            .is_some_and(|identity| !known.contains(identity))
        {
            self.expanded = None;
        }
        self.approved.retain(|identity| known.contains(identity));
    }

    /// The journaled edits, oldest first.
    #[must_use]
    pub fn entries(&self) -> &[JournalEntry] {
        &self.entries
    }

    /// The journal rewound `id`: its entry leaves the list, with
    /// whatever was recorded on it.
    pub fn remove(&mut self, id: &str) {
        self.entries.retain(|entry| entry.id != id);
        self.retain_flags();
        self.refused.remove(id);
    }

    /// The workbench refused to rewind `id`, in its own words — a
    /// conflict (a later change touched the file), an op already
    /// undone, or another session's. Recorded on the entry so the card
    /// says why the change is still there; whether the id is journaled
    /// at all, which is what tells a refusal on a listed change from a
    /// refusal on an edit the journal never held.
    pub fn refuse(&mut self, id: &str, reason: &str) -> bool {
        if !self.entries.iter().any(|entry| entry.id == id) {
            return false;
        }
        self.refused.insert(id.to_owned(), reason.to_owned());
        true
    }

    /// Why the last rewind of `id` was refused, while the entry stands.
    #[must_use]
    pub fn refusal(&self, id: &str) -> Option<&str> {
        self.refused.get(id).map(String::as_str)
    }

    /// A click on the entry at `index`: its diff expands, or folds when
    /// it already shows — one entry expands at a time.
    pub fn toggle(&mut self, index: usize) {
        let Some(entry) = self.entries.get(index) else {
            return;
        };
        let identity = entry.identity();
        self.expanded = if self.expanded.as_deref() == Some(identity.as_str()) {
            None
        } else {
            Some(identity)
        };
    }

    /// Whether the file row's diff shows, addressed by JournalEntry::identity.
    #[must_use]
    pub fn is_expanded(&self, id: &str) -> bool {
        self.expanded.as_deref() == Some(id)
    }

    /// \[Approve\] on the entry at `index`: marked reviewed, purely in
    /// the cockpit.
    pub fn approve(&mut self, index: usize) {
        if let Some(entry) = self.entries.get(index) {
            self.approved.insert(entry.identity());
        }
    }

    /// Whether the human approved the file row, addressed by JournalEntry::identity.
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

// ── The interaction over the journal ────────────────────────────────────

/// Fold a workbench answer into the model. File, search and map results
/// open the viewer pane in the inspector; run results join the Session
/// timeline; the journal replaces the list; an undo takes the operation
/// off the list and marks its turn card undone. A refusal is an error
/// line on the stage, and a refused *undo* is also recorded on the
/// entry it named — the conflict (a later change touched the file) is
/// the entry's to show, not the transcript's alone.
pub(super) fn fold(model: &mut Model, event: WorkbenchEvent) {
    match event {
        WorkbenchEvent::Opened { path, content } => {
            model.session.turn.push_note(&format!(
                "opened {path} · {} lines",
                content.lines().count()
            ));
            let lines = content.lines().map(str::to_owned).collect();
            model.trace.close_viewer();
            model.changes.open_viewer(&path, lines);
            model.pane = Pane::Inspector;
            model.inspector_scroll = 0;
            model.show_changes();
        }
        WorkbenchEvent::Undone { id } => {
            model.session.turn.undo_edit(&id);
            model.changes.remove(&id);
        }
        WorkbenchEvent::Journal { entries } => model.changes.set_entries(entries),
        WorkbenchEvent::Found { query, hits } => {
            let title = format!("search {query} · {} hits", hits.len());
            model.trace.close_viewer();
            model.changes.open_viewer(&title, hits);
            model.pane = Pane::Inspector;
            model.inspector_scroll = 0;
            model.show_changes();
        }
        WorkbenchEvent::Mapped { lines } => {
            model.trace.close_viewer();
            model.changes.open_viewer("map", lines);
            model.pane = Pane::Inspector;
            model.inspector_scroll = 0;
            model.show_changes();
        }
        WorkbenchEvent::Ran { command, rows } => {
            model.changes.set_runs(&command, rows);
            model.show_changes();
        }
        WorkbenchEvent::Failed { cmd, error } => {
            if let WorkbenchCmd::Undo { id } = &cmd {
                model.changes.refuse(id, &error);
            }
            model.session.turn.push_error(&format!(
                "workbench {}: {error}",
                crate::workbench::describe(&cmd)
            ));
        }
    }
    if model.changes.viewer().is_some() {
        model.pane = Pane::Inspector;
    }
}

/// Select a journal row by its stable identity and reveal its details.
/// Each surface draws that selection in its own layout.
pub(super) fn row_click(model: &mut Model, index: usize) {
    if let Some(entry) = model.changes.entries().get(index) {
        let selected = Selected::Change(entry.identity());
        model.changes.toggle(index);
        model.inspect(selected);
        if !model.uses_flight_recorder() {
            model.pane = Pane::Stage;
        }
        super::update::reveal_session_selection(model);
    }
}

/// \[Open\] on the change at `index` in the journal list: read its file
/// into the viewer pane.
pub(super) fn open_click(model: &Model, index: usize) -> Vec<Cmd> {
    let Some(entry) = model.changes.entries().get(index) else {
        return Vec::new();
    };
    vec![Cmd::Workbench(WorkbenchCmd::Open {
        path: entry.path.clone(),
    })]
}

/// \[Undo\] on the change at `index`: rewind its journal step — the
/// [`WorkbenchEvent::Undone`] answer takes it off the list, and a
/// [`WorkbenchEvent::Failed`] answer lands on it as a refusal.
pub(super) fn undo_click(model: &Model, index: usize) -> Vec<Cmd> {
    let Some(entry) = model.changes.entries().get(index) else {
        return Vec::new();
    };
    vec![Cmd::Workbench(WorkbenchCmd::Undo {
        id: entry.id.clone(),
    })]
}

/// \[Approve\] on the change at `index`: the entry is marked reviewed,
/// and — when a policy held the edit for review and put its
/// diff-approval card on the Needs-you dock — that card is answered
/// with it, so the human reviews the diff once rather than twice. No
/// card for the file means the approval is the pure reviewed-mark it
/// always was.
pub(super) fn approve_click(model: &mut Model, index: usize) -> Vec<Cmd> {
    model.changes.approve(index);
    let Some(path) = model
        .changes
        .entries()
        .get(index)
        .map(|entry| entry.path.clone())
    else {
        return Vec::new();
    };
    decision::approve_edit(model, &path)
}

/// \[Open\] or \[Undo\] on the selected edit card's inspector verbs —
/// the turn's edit line is the agent's intent, and its Undo names the
/// journal by the call's id. A card already undone answers nothing.
pub(super) fn selected_edit_click(model: &Model, undo: bool) -> Vec<Cmd> {
    let Some(Selected::Tool(id)) = model.selected() else {
        return Vec::new();
    };
    let Some(line) = model.turn().tool(id) else {
        return Vec::new();
    };
    let crate::turn::LineKind::Edit { path, undone, .. } = line.kind() else {
        return Vec::new();
    };
    if undo && *undone {
        return Vec::new();
    }
    vec![Cmd::Workbench(if undo {
        WorkbenchCmd::Undo { id: id.clone() }
    } else {
        WorkbenchCmd::Open { path: path.clone() }
    })]
}

/// Open or undo the edit identified by its visible inline button row.
pub(super) fn edit_click(model: &mut Model, index: usize, undo: bool) -> Vec<Cmd> {
    let row = model.top_row().saturating_add(index);
    let Some((id, path, undone)) = model.session.turn.edit_at_row(row) else {
        return Vec::new();
    };
    let cmd = if undo {
        if undone {
            return Vec::new();
        }
        WorkbenchCmd::Undo { id: id.to_owned() }
    } else {
        WorkbenchCmd::Open {
            path: path.to_owned(),
        }
    };
    vec![Cmd::Workbench(cmd)]
}

#[cfg(test)]
#[path = "changes.tests.rs"]
mod tests;
