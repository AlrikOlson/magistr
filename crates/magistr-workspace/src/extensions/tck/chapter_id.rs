use serde::Serialize;

/// The four chapters, in report order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TckChapterId {
    Interface,
    Capabilities,
    Resources,
    LangPackFuzz,
}

impl TckChapterId {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Interface => "interface",
            Self::Capabilities => "capabilities",
            Self::Resources => "resources",
            Self::LangPackFuzz => "lang-pack-fuzz",
        }
    }
}
