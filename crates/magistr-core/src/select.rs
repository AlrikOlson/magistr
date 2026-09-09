//! Selection list types — data model for option lists and checklists.
//!
//! These types represent the data and navigation state for selection UIs.
//! Frontend-specific rendering (ratatui widgets, HTML components) is
//! implemented in the respective frontend crates.

/// A single item in a select list.
#[derive(Debug, Clone)]
pub struct SelectItem {
    /// Display label
    pub label: String,
    /// Value returned when selected
    pub value: String,
    /// Optional description text (shown below label in card mode)
    pub description: Option<String>,
}

impl SelectItem {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            description: None,
        }
    }

    /// Create an item with a description.
    pub fn with_description(
        label: impl Into<String>,
        value: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            description: Some(description.into()),
        }
    }

    /// Convenience: create an item where label and value are the same.
    pub fn simple(label: impl Into<String>) -> Self {
        let s = label.into();
        Self {
            label: s.clone(),
            value: s,
            description: None,
        }
    }
}

/// A list of selectable items with cursor state.
#[derive(Debug, Clone)]
pub struct SelectList {
    /// Items to choose from.
    items: Vec<SelectItem>,
    /// Currently selected index.
    selected: usize,
    /// Whether this list currently has input focus.
    pub focused: bool,
    /// Display label for the list.
    label: String,
    /// Previous selected index (for animation — `None` until first move).
    /// Set on `move_up()`/`move_down()` before updating `selected`.
    previous_selected: Option<usize>,
    /// Whether the selection changed this tick (cleared after one render tick).
    pub selection_changed: bool,
}

impl SelectList {
    /// Create a new select list with the given label and items.
    pub fn new(label: impl Into<String>, items: Vec<SelectItem>) -> Self {
        Self {
            items,
            selected: 0,
            focused: false,
            label: label.into(),
            previous_selected: None,
            selection_changed: false,
        }
    }

    /// Get the label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Get the currently selected item, if any.
    pub fn selected_item(&self) -> Option<&SelectItem> {
        self.items.get(self.selected)
    }

    /// Get the selected value, if any.
    pub fn selected_value(&self) -> Option<&str> {
        self.selected_item().map(|i| i.value.as_str())
    }

    /// Move selection up (wraps to last item at top).
    pub fn move_up(&mut self) {
        if !self.items.is_empty() {
            self.previous_selected = Some(self.selected);
            self.selection_changed = true;
            if self.selected == 0 {
                self.selected = self.items.len() - 1; // wrap
            } else {
                self.selected -= 1;
            }
        }
    }

    /// Move selection down (wraps to first item at bottom).
    pub fn move_down(&mut self) {
        if !self.items.is_empty() {
            self.previous_selected = Some(self.selected);
            self.selection_changed = true;
            self.selected = (self.selected + 1) % self.items.len();
        }
    }

    /// Set selection by value. Returns true if found.
    pub fn set_selected_value(&mut self, value: &str) -> bool {
        if let Some(idx) = self.items.iter().position(|i| i.value == value) {
            self.selected = idx;
            true
        } else {
            false
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the list is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Select item by 0-based index. Returns true if index is valid.
    ///
    /// Used for numbered shortcut keys (press 1-9 to select).
    pub fn select_by_index(&mut self, index: usize) -> bool {
        if index < self.items.len() {
            self.selected = index;
            true
        } else {
            false
        }
    }

    /// Get all items.
    pub fn items(&self) -> &[SelectItem] {
        &self.items
    }

    /// Get the selected index.
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// Get the previous selected index (before the last move).
    pub fn previous_selected(&self) -> Option<usize> {
        self.previous_selected
    }

    /// Clear the selection-changed flag (call after triggering animation).
    pub fn clear_selection_changed(&mut self) {
        self.selection_changed = false;
    }
}

#[cfg(test)]
#[path = "select.tests.rs"]
mod tests;
