/// Manifest-requested guest ceilings; each clamps the host policy downward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PluginLimits {
    pub fuel: Option<u64>,
    pub memory_bytes: Option<usize>,
}
