use magistr_cockpit::cockpit::Msg;
use std::{future::Future, pin::Pin};

/// Application-owned observation destination. Posting never waits on a surface.
pub trait ObservationSink: Send + Sync + std::fmt::Debug {
    fn send(&self, message: Msg) -> bool;
    fn is_closed(&self) -> bool;
    fn closed(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}
