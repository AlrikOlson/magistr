/// Pinned reference port signature versions; concrete native effects land separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortVersion {
    Engine,
    Index,
    Journal,
    Watcher,
    Sandbox,
    Formatter,
    Resolver,
}

impl PortVersion {
    pub const ALL: [Self; 7] = [
        Self::Engine,
        Self::Index,
        Self::Journal,
        Self::Watcher,
        Self::Sandbox,
        Self::Formatter,
        Self::Resolver,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Engine => "engine",
            Self::Index => "index",
            Self::Journal => "journal",
            Self::Watcher => "watcher",
            Self::Sandbox => "sandbox",
            Self::Formatter => "formatter",
            Self::Resolver => "resolver",
        }
    }

    #[must_use]
    pub const fn signature_version(self) -> u64 {
        match self {
            Self::Engine | Self::Resolver => 2,
            Self::Index => 9,
            Self::Journal => 4,
            Self::Watcher | Self::Sandbox | Self::Formatter => 1,
        }
    }
}
