/// Internal fault injection crosses the real native filesystem/journal stages.
#[derive(Default)]
pub(super) struct Faults {
    pub swap_at: Option<usize>,
    pub compensation_at: Option<usize>,
    pub after_swap_at: Option<usize>,
    pub after_compensation_at: Option<usize>,
    pub resolve: bool,
}
