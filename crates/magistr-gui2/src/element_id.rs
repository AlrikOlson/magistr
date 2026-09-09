/// The stable identity an interactive element wears, so hover, press
/// and focus can survive a rebuild of the tree.
///
/// Its own newtype rather than the cockpit's widget identity: a widget
/// names a place in the cockpit's shared Model, while an element id
/// names a spot of *this window's* component tree — the two vocabularies
/// meet only at the Msg fold, where an element folds the same messages
/// the cockpit folds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ElementId(pub u64);
