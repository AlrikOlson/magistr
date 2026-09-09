use serde::{Deserialize, Serialize};

/// The two equal voting positions. There is no tie-breaking third voter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Peer {
    Astra,
    Fable,
}
