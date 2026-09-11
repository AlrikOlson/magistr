/// How a restoration plan treats bytes changed since the journal operation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UndoConflictPolicy {
    #[default]
    Surface,
    KeepCurrent,
    Force,
}
