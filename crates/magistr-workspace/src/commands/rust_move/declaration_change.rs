use super::RustDeclarationKind;

/// The declaring `mod old;` items that were rewritten, and where the new
/// declaration landed (never synthesized: `None` when no parent file exists).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustDeclarationChange {
    pub kind: RustDeclarationKind,
    pub from: Vec<String>,
    pub to: Option<String>,
}
