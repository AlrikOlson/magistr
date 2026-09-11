/// Where a crate-relative `.rs` file sits in the module tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ModuleLocation {
    /// Root files whose tree contains the file; several when lib.rs and main.rs share src/.
    pub roots: Vec<String>,
    /// The directory the roots live in (crate-relative).
    pub root_dir: String,
    /// Module path segments below `crate`; empty for a root file.
    pub segments: Vec<String>,
    /// Candidate files that declare the last segment (the file's parent module).
    pub parents: Vec<String>,
}
