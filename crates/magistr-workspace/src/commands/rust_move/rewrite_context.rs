use std::collections::BTreeSet;

/// How one file's paths are resolved and re-emitted for a move.
pub(super) struct RewriteContext<'c> {
    /// The file's module path for resolving (None = unknown: only crate:: chains resolve).
    pub resolve_module: Option<&'c [String]>,
    /// The file's module path for re-emitting (differs from resolve_module only for the moved file).
    pub emit_module: Option<&'c [String]>,
    /// Module names declared in this file before the move (resolution).
    pub declared_before: &'c BTreeSet<String>,
    /// Module names declared in this file after the move (uniform re-emission).
    pub declared_after: &'c BTreeSet<String>,
    pub old_module: &'c [String],
    pub new_module: &'c [String],
}
