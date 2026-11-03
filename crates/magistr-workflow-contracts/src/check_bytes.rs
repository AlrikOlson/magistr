//! What a running check wrote.

use serde::{Deserialize, Serialize};

/// One task a check's command wrote to its pty — escapes and all, in the
/// order it wrote them — keyed by the check's name.
///
/// Both check callers speak it: the loop driver posts it on the cockpit's
/// inbox as a local [`NativeEvent`](super::NativeEvent), `magistr mcp`
/// emits it on the broadcast socket under the `magistr` family
/// ([`MagistrFrame::CheckBytes`](super::MagistrFrame::CheckBytes)), and the
/// cockpit folds either into the terminal store the pane paints from. The
/// bytes travel as a JSON byte array — the same shape an agent terminal's
/// bytes take on the server relay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckBytes {
    /// The check's name (`check`, `test`, `lint`, a custom check).
    pub check: String,
    /// The task, raw.
    pub bytes: Vec<u8>,
}
