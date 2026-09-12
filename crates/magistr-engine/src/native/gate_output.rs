//! What a running quality gate wrote.

use serde::{Deserialize, Serialize};

/// One chunk a gate's command wrote to its pty — escapes and all, in the
/// order it wrote them — keyed by the gate's name.
///
/// Both gate callers speak it: the loop driver posts it on the cockpit's
/// inbox as a local [`NativeEvent`](super::NativeEvent), `magistr mcp-serve`
/// emits it on the broadcast socket under the `magistr` family
/// ([`MagistrFrame::GateOutput`](super::MagistrFrame::GateOutput)), and the
/// cockpit folds either into the terminal store the pane paints from. The
/// bytes travel as a JSON byte array — the same shape an agent terminal's
/// bytes take on the server relay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateOutput {
    /// The gate's name (`check`, `test`, `lint`, a custom gate).
    pub gate: String,
    /// The chunk, raw.
    pub bytes: Vec<u8>,
}
