/// How the declaring `mod` item changed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RustDeclarationKind {
    Rename,
    Move,
    RemoveOnly,
}
