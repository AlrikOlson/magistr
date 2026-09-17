//! Host-owned workflow admission, attached ports and snapshot delivery.
//! Shared interfaces receive events through their semantic ports and inbox.

mod attached_port;
mod client;
mod error;
mod feed;
mod feed_link;
mod native_port;
mod reseed;

pub use attached_port::AttachedEnginePort;
pub use client::EngineClient;
pub use error::EngineError;
pub use native_port::NativeEnginePort;
