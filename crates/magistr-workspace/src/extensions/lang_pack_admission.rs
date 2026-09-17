use super::{AdmittedLangPack, LangPackStatus};

/// The owner's admission of the installed lang packs: the packs every
/// incarnation registers, and the inventory entries refused before any
/// worker was involved (no directory, unreadable manifest, missing
/// component), which stay status rows so the refusal is observable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LangPackAdmission {
    pub packs: Vec<AdmittedLangPack>,
    pub refused: Vec<LangPackStatus>,
}

impl LangPackAdmission {
    /// True when no pack needs registering: the worker then needs no
    /// restore step and a workspace without packs pays nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.packs.is_empty()
    }
}
