use magistr_cockpit::cockpit::Msg;
use std::{future::Future, pin::Pin};

/// Application-owned observation destination. Posting never waits on a surface.
pub trait ObservationSink: Send + Sync + std::fmt::Debug {
    fn send(&self, message: Msg) -> bool;
    /// Select an already bound context before admitting its next input.
    fn activate(&self, _id: &str) -> Result<(), String> {
        Ok(())
    }
    /// Record against a producer's immutable owner, even when another context is active.
    fn send_for(&self, _id: &str, message: Msg) -> bool {
        self.send(message)
    }
    fn is_closed(&self) -> bool;
    fn closed(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}
