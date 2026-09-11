/// An importer, parent or manifest write; never the moved file itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustMoveWrite {
    pub path: String,
    pub before: String,
    pub content: String,
}
