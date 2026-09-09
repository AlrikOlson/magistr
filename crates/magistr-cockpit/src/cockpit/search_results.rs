//! What a ⌘⇧F query found.

use super::search_hit::SearchHit;

/// The answer to a query: the rows the overlay lists, grouped by kind
/// in [`SearchKind::ALL`](super::SearchKind::ALL)'s order, and the
/// total number of hits across every kind — which is larger than the
/// list whenever a kind was cut to its strongest few, so the count
/// says what the query *found* and the rows what the overlay *shows*.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SearchResults {
    /// Every hit, counted before any cut.
    pub total: usize,
    /// The rows shown, kind by kind.
    pub rows: Vec<SearchHit>,
}

impl SearchResults {
    /// The count as the field prints it — `6 results`, `1 result`.
    #[must_use]
    pub fn count_text(&self) -> String {
        let plural = if self.total == 1 { "" } else { "s" };
        format!("{} result{plural}", self.total)
    }
}
