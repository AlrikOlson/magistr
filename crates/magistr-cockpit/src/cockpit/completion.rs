//! The completion popup over the prompt.

use super::completion_kind::CompletionKind;

/// How many candidates the popup shows.
const MAX_ITEMS: usize = 6;

/// The commands the cockpit completes: the scaffolded skill set every
/// magistr project carries, and the palette commands the cockpit
/// answers itself — the workbench's `/search`, `/map`, `/read` and
/// `/run`, the think engine's `/pin`, `/unpin` and `/checkpoint`, and
/// the ledger's `/checkpoint-now`, `/restore` and `/fork`.
const COMMANDS: [&str; 15] = [
    "/checkpoint",
    "/checkpoint-now",
    "/fork",
    "/init",
    "/map",
    "/next",
    "/next-loop",
    "/pin",
    "/read",
    "/restore",
    "/roadmap",
    "/run",
    "/search",
    "/unpin",
    "/validate",
];

/// The completion popup: what triggered it, where the trigger sits in
/// the prompt, the candidates matching the query so far, and which one
/// is selected.
///
/// The query is the prompt text between the trigger and the cursor;
/// [`Completion::refresh`] re-filters after every edit and says when
/// the popup should close instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    kind: CompletionKind,
    anchor: usize,
    items: Vec<String>,
    selected: usize,
}

impl Completion {
    /// File completion for the `@` at byte `anchor`.
    #[must_use]
    pub const fn file(anchor: usize) -> Self {
        Self::new(CompletionKind::File, anchor)
    }

    /// Command completion for the `/` at byte `anchor`.
    #[must_use]
    pub const fn command(anchor: usize) -> Self {
        Self::new(CompletionKind::Command, anchor)
    }

    const fn new(kind: CompletionKind, anchor: usize) -> Self {
        Self {
            kind,
            anchor,
            items: Vec::new(),
            selected: 0,
        }
    }

    /// Whether the candidates attach project files.
    #[must_use]
    pub const fn is_file(&self) -> bool {
        matches!(self.kind, CompletionKind::File)
    }

    /// Where the trigger character sits in the prompt.
    #[must_use]
    pub const fn anchor(&self) -> usize {
        self.anchor
    }

    /// The candidates matching the query, at most [`MAX_ITEMS`].
    #[must_use]
    pub fn items(&self) -> &[String] {
        &self.items
    }

    /// The selected candidate's index.
    #[must_use]
    pub const fn selected(&self) -> usize {
        self.selected
    }

    /// Move the selection up, wrapping at the top.
    pub fn up(&mut self) {
        if !self.items.is_empty() {
            self.selected = self.selected.checked_sub(1).unwrap_or(self.items.len() - 1);
        }
    }

    /// Move the selection down, wrapping at the bottom.
    pub fn down(&mut self) {
        if !self.items.is_empty() {
            self.selected = (self.selected + 1) % self.items.len();
        }
    }

    /// Select the candidate at `index` (a click on its row).
    pub fn select(&mut self, index: usize) {
        if index < self.items.len() {
            self.selected = index;
        }
    }

    /// Re-filter against the prompt; `false` when the popup should
    /// close instead — the cursor left the query, the trigger is gone,
    /// whitespace joined the query, or nothing matches.
    pub fn refresh(&mut self, text: &str, cursor: usize, files: &[String]) -> bool {
        let trigger = self.kind.trigger();
        let intact = text
            .get(self.anchor..)
            .is_some_and(|t| t.starts_with(trigger));
        let query = intact
            .then(|| text.get(self.anchor + trigger.len_utf8()..cursor))
            .flatten();
        let Some(query) = query else {
            return false;
        };
        if query.chars().any(char::is_whitespace) {
            return false;
        }
        let query = query.to_lowercase();
        self.items = match self.kind {
            CompletionKind::File => files
                .iter()
                .filter(|file| file.to_lowercase().contains(&query))
                .take(MAX_ITEMS)
                .cloned()
                .collect(),
            CompletionKind::Command => COMMANDS
                .iter()
                .filter(|command| command[1..].starts_with(&query))
                .take(MAX_ITEMS)
                .map(|command| (*command).to_owned())
                .collect(),
        };
        if self.items.is_empty() {
            return false;
        }
        self.selected = self.selected.min(self.items.len() - 1);
        true
    }

    /// What accepting puts over `anchor..cursor`: the selected
    /// candidate — its trigger kept for a file — and a trailing space.
    /// `None` when nothing is selected (an empty popup).
    #[must_use]
    pub fn replacement(&self) -> Option<String> {
        let item = self.items.get(self.selected)?;
        Some(match self.kind {
            CompletionKind::File
                if item
                    .chars()
                    .any(|c| c.is_whitespace() || c == '"' || c == '\\') =>
            {
                format!(
                    "@{} ",
                    serde_json::to_string(item).expect("a file name is valid JSON")
                )
            }
            CompletionKind::File => format!("@{item} "),
            CompletionKind::Command => format!("{item} "),
        })
    }
}

#[cfg(test)]
#[path = "completion.tests.rs"]
mod tests;
