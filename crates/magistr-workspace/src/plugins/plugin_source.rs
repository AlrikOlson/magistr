/// Where an installed plugin came from. Anything but the registry is a
/// sideload and stays untrusted whatever its record claims.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginSource {
    Registry,
    Sideload,
}

impl PluginSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Registry => "registry",
            Self::Sideload => "sideload",
        }
    }
}
