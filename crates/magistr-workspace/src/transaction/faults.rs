/// Internal fault injection crosses the real native filesystem/journal stages.
#[derive(Default)]
pub(super) struct Faults {
    pub swap_at: Option<usize>,
    pub compensation_at: Option<usize>,
    pub after_swap_at: Option<usize>,
    pub after_compensation_at: Option<usize>,
    pub resolve: bool,
    /// Runs after file compensation and before batch-created directories are
    /// removed, standing in for a concurrent writer.
    pub before_directory_cleanup: Option<Box<dyn Fn() + Send + Sync>>,
}
