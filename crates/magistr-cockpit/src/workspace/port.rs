//! What the cockpit asks of a workspace.

use super::run_row::RunRow;
use crate::workspace::journal_entry::JournalEntry;

/// A workspace the cockpit acts through without the agent: the Changes
/// tab tails [`WorkspacePort::journal`], the edit and change cards'
/// buttons read and rewind, and the prompt's palette commands reach
/// search, map and run. Errors are words for the stage, not codes.
/// `Send + Sync` because the session task holds the port across its
/// awaits.
pub trait WorkspacePort: Send + Sync {
    /// The file's current content.
    ///
    /// # Errors
    /// The workspace's reason the file cannot be read.
    fn read(&mut self, path: &str) -> Result<String, String>;

    /// Rewind the journal entry behind the edit `id`.
    ///
    /// # Errors
    /// The workspace's reason the rewind did not happen.
    fn undo(&mut self, id: &str) -> Result<(), String>;

    /// The write journal as it stands, oldest edit first.
    ///
    /// # Errors
    /// The workspace's reason the journal cannot be read.
    fn journal(&mut self) -> Result<Vec<JournalEntry>, String>;

    /// Ranked hits for `query`, one line each.
    ///
    /// # Errors
    /// The workspace's reason the search did not run.
    fn search(&mut self, query: &str) -> Result<Vec<String>, String>;

    /// The workspace map, one line per entry.
    ///
    /// # Errors
    /// The workspace's reason the map cannot be drawn.
    fn map(&mut self) -> Result<Vec<String>, String>;

    /// Run the declared `command`; rows in the runner's order.
    ///
    /// # Errors
    /// The workspace's reason the command did not run — an undeclared
    /// command included.
    fn run(&mut self, command: &str) -> Result<Vec<RunRow>, String>;
}
