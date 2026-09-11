/// Persisted row counts without materializing the index contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheCounts {
    pub files: u64,
    pub symbols: u64,
    pub imports: u64,
    pub test_ranges: u64,
    pub content: u64,
}
