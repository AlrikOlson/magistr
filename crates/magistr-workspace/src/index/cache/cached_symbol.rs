use crate::worker::WorkerSymbol;

use super::IndexedFile;

/// A symbol paired with the freshness anchor of the file that supplied it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedSymbol {
    pub file: IndexedFile,
    pub symbol: WorkerSymbol,
}
