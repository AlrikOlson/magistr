use crate::element::Element;

/// One section of the kitchen sink — a titled run of catalog entries,
/// each naming the component modules it exhibits.
///
/// The names are load-bearing: the completeness test derives the set of
/// component modules from `src/` and requires every one to appear in
/// some section's entries, so a component that skips the sink fails by
/// name. An entry's element is the very tree the sink renders — the
/// roster walks what the window shows, not a parallel list that could
/// drift.
pub struct SinkSection {
    /// The section's title, worn as a cap — the specimen sheet's own
    /// section headings.
    pub title: &'static str,
    /// Each entry: the component modules it exhibits, and the tree.
    pub entries: Vec<(&'static [&'static str], Element)>,
}
