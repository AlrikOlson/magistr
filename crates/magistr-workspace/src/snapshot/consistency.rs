use serde::{Deserialize, Serialize};

/// What a snapshot's bytes are allowed to claim about the inputs a consumer
/// ran against.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Consistency {
    /// A live observation of a shared tree. Anything may have changed between
    /// two reads, and two equal digests prove only that both observations
    /// saw the same bytes — never that nothing happened in between. This is
    /// the only consistency a live workspace can offer.
    Optimistic,
    /// Bytes copied into a directory that nothing else writes, re-read and
    /// found identical to the origin observation. A consumer that executes
    /// inside that directory ran against exactly these inputs.
    Immutable,
}

impl Consistency {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Optimistic => "optimistic",
            Self::Immutable => "immutable",
        }
    }
}
