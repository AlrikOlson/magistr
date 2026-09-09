//! The think-and-ship engines, in-process, and their live feed.
//!
//! [`EngineClient::open`] opens the project's store through
//! magistr-engine's [`NativeEngines`](magistr_engine::NativeEngines) —
//! before the terminal is taken over, so a store that cannot be opened is
//! reported on a normal stdout and never inside the alternate screen —
//! and [`EngineClient::subscribe`] attaches to the broadcast socket
//! `magistr mcp-serve` binds, relaying every frame and every change of
//! attachment onto the inbox as [`Msg::Engine`](crate::cockpit::Msg::Engine).

pub mod apply;
pub mod attached;
pub mod client;
pub mod error;
pub mod event;
pub mod feed;
pub mod native_port;
pub mod port;
pub mod reseed;
pub mod seed;

pub use apply::apply;
pub use attached::FeedAttached;
pub use client::EngineClient;
pub use error::EngineError;
pub use event::EngineEvent;
pub use feed::spawn_feed;
pub use native_port::NativeEnginePort;
pub use port::EnginePort;
pub use reseed::spawn_reseed;
pub use seed::StoreSeed;
