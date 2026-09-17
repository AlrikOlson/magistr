/// One admitted entry in the deterministic live source walk.
#[derive(Debug, Clone)]
pub(crate) struct SourceEntry {
    pub path: String,
    pub is_dir: bool,
    pub junk: bool,
}
