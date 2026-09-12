//! What a running quality gate wrote, as a broadcast frame.

use serde::{Deserialize, Serialize};

use super::broadcast_frame::NativeFrame;

/// One chunk a gate's command wrote to its pty — escapes and all, in the
/// order it wrote them — keyed by the gate's name.
///
/// Both gate callers speak it: the loop driver posts it on the cockpit's
/// inbox as an engine frame, `magistr mcp-serve` emits it on the
/// broadcast socket the cockpit's engine feed relays, and the cockpit
/// folds either into the terminal store the pane paints from. It rides
/// the `ship` family (a gate's record lands there too) under the
/// `gate_output` kind, the bytes as a JSON byte array — the same shape
/// an agent terminal's bytes take on the server relay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateOutput {
    /// The gate's name (`check`, `test`, `lint`, a custom gate).
    pub gate: String,
    /// The chunk, raw.
    pub bytes: Vec<u8>,
}

impl GateOutput {
    /// The family the frame rides.
    pub const FAMILY: &'static str = "ship";
    /// The frame's `type` discriminator.
    pub const KIND: &'static str = "gate_output";

    /// This chunk as the frame the socket would carry.
    #[must_use]
    pub fn frame(&self) -> NativeFrame {
        NativeFrame {
            family: Self::FAMILY.to_owned(),
            kind: Self::KIND.to_owned(),
            payload: serde_json::json!({
                "family": Self::FAMILY,
                "type": Self::KIND,
                "gate": self.gate,
                "bytes": self.bytes,
            }),
        }
    }

    /// The chunk a frame carries, when it is one of these.
    #[must_use]
    pub fn from_frame(frame: &NativeFrame) -> Option<Self> {
        if frame.family != Self::FAMILY || frame.kind != Self::KIND {
            return None;
        }
        let gate = frame.payload.get("gate")?.as_str()?.to_owned();
        let bytes = serde_json::from_value(frame.payload.get("bytes")?.clone()).ok()?;
        Some(Self { gate, bytes })
    }
}

#[cfg(test)]
#[path = "gate_output.tests.rs"]
mod tests;
