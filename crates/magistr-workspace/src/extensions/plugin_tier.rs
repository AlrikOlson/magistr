/// Registry trust tier; honored only on registry entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginTier {
    Blessed,
    Verified,
}

impl PluginTier {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Blessed => "blessed",
            Self::Verified => "verified",
        }
    }
}
