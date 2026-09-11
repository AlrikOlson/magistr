/// One `mod name;` item with its leading attributes and doc comments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DeclarationItem {
    /// Index of the first line of the item (attributes/docs included), 0-based.
    pub start_line: usize,
    /// Index of the `mod name;` line, 0-based.
    pub decl_line: usize,
    pub has_path_attr: bool,
    pub is_pub: bool,
}
