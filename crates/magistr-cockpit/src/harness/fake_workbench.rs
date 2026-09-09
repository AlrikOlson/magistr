//! A scripted workbench for the harness.

use std::collections::HashMap;

use crate::workbench::{JournalEntry, RunRow, WorkbenchPort};

/// A [`WorkbenchPort`] made of scripts: files it will answer a read
/// with, a journal of edits it will list and rewind (an undo restores
/// the scripted pre-edit content, so the harness can watch the change
/// leave the disk), run rows per declared command — and a log of every
/// call for a test to assert the round-trip on. Search and map derive
/// from the files; an unknown path, id or command is refused the way
/// the real workbench refuses.
#[derive(Debug, Default)]
pub struct FakeWorkbench {
    files: HashMap<String, String>,
    journal: Vec<JournalEntry>,
    reverts: HashMap<String, String>,
    runs: HashMap<String, Vec<RunRow>>,
    calls: Vec<String>,
}

impl FakeWorkbench {
    /// An empty workbench: every read, undo and run is refused.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The workbench with `path` answering a read with `content`.
    #[must_use]
    pub fn with_file(mut self, path: &str, content: &str) -> Self {
        self.files.insert(path.to_owned(), content.to_owned());
        self
    }

    /// The workbench with the bare edit `id` in its journal, ready to
    /// rewind — enough for a card's [Undo] round-trip.
    #[must_use]
    pub fn with_edit(mut self, id: &str) -> Self {
        self.journal.push(JournalEntry {
            id: id.to_owned(),
            ..JournalEntry::default()
        });
        self
    }

    /// The workbench with a full journal `entry`; rewinding it restores
    /// the entry's file to `revert_to`.
    #[must_use]
    pub fn with_change(mut self, entry: JournalEntry, revert_to: &str) -> Self {
        self.reverts.insert(entry.id.clone(), revert_to.to_owned());
        self.journal.push(entry);
        self
    }

    /// The workbench answering a run of `command` with `rows`.
    #[must_use]
    pub fn with_run(mut self, command: &str, rows: Vec<RunRow>) -> Self {
        self.runs.insert(command.to_owned(), rows);
        self
    }

    /// Every call, in order, as `read <path>` / `undo <id>` /
    /// `journal` / `search <q>` / `map` / `run <command>` lines.
    #[must_use]
    pub fn calls(&self) -> &[String] {
        &self.calls
    }

    /// The journal as it stands — an undo removes its entry.
    #[must_use]
    pub fn journal_entries(&self) -> &[JournalEntry] {
        &self.journal
    }

    /// The file at `path` as the workbench holds it — the harness's
    /// witness that an undo restored the pre-edit content.
    #[must_use]
    pub fn file(&self, path: &str) -> Option<&str> {
        self.files.get(path).map(String::as_str)
    }
}

impl WorkbenchPort for FakeWorkbench {
    fn read(&mut self, path: &str) -> Result<String, String> {
        self.calls.push(format!("read {path}"));
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| format!("no such file: {path}"))
    }

    fn undo(&mut self, id: &str) -> Result<(), String> {
        self.calls.push(format!("undo {id}"));
        let Some(at) = self.journal.iter().position(|known| known.id == id) else {
            return Err(format!("no journal entry for {id}"));
        };
        let entry = self.journal.remove(at);
        if let Some(old) = self.reverts.remove(id) {
            self.files.insert(entry.path, old);
        }
        Ok(())
    }

    fn journal(&mut self) -> Result<Vec<JournalEntry>, String> {
        self.calls.push("journal".to_owned());
        Ok(self.journal.clone())
    }

    fn search(&mut self, query: &str) -> Result<Vec<String>, String> {
        self.calls.push(format!("search {query}"));
        let mut paths: Vec<&String> = self.files.keys().collect();
        paths.sort();
        let mut hits = Vec::new();
        for path in paths {
            for (number, line) in self.files[path].lines().enumerate() {
                if line.contains(query) {
                    hits.push(format!("{path}:{}: {}", number + 1, line.trim()));
                }
            }
        }
        Ok(hits)
    }

    fn map(&mut self) -> Result<Vec<String>, String> {
        self.calls.push("map".to_owned());
        let mut paths: Vec<String> = self.files.keys().cloned().collect();
        paths.sort();
        Ok(paths)
    }

    fn run(&mut self, command: &str) -> Result<Vec<RunRow>, String> {
        self.calls.push(format!("run {command}"));
        self.runs
            .get(command)
            .cloned()
            .ok_or_else(|| format!("no declared command: {command}"))
    }
}

#[cfg(test)]
#[path = "fake_workbench.tests.rs"]
mod tests;
