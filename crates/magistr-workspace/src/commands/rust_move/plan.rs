use super::{RustDeclarationChange, RustMoveWrite, RustUnrewritten};

/// Every rewrite a Rust-aware move needs; nothing here writes anything.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustMovePlan {
    pub crate_dir: String,
    pub old_module: Option<Vec<String>>,
    pub new_module: Option<Vec<String>>,
    pub declaration: Option<RustDeclarationChange>,
    /// The moved file's content after its own rewrites (unchanged when none).
    pub moved_content: String,
    pub moved_rewrites: usize,
    pub writes: Vec<RustMoveWrite>,
    pub path_rewrites: Vec<(String, usize)>,
    pub manifest: Option<(String, usize)>,
    pub unrewritten: Vec<RustUnrewritten>,
    /// Set when no module rewrite was attempted; the move proceeds as a plain file move.
    pub skipped: Option<String>,
    pub notes: Vec<String>,
}
