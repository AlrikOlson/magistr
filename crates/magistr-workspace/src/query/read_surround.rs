/// Additional context returned around a selected symbol.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReadSurround {
    None,
    #[default]
    Enclosing,
    Imports,
}
