/// One admitted entry in the deterministic live source walk.
pub(super) struct SourceEntry {
    pub path: String,
    pub is_dir: bool,
    pub language: Option<String>,
    pub junk: bool,
}
