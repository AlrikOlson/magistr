//! The think-and-ship engines, in-process, and their live feed.
//!
//! [`EngineClient::with_config`] admits the process as the project's host
//! through magistr-engine's [`ProjectAuthority`](magistr_engine::ProjectAuthority)
//! on a writer configuration carrying the project's verification evidence
//! capability, and holds the writer, [`NativeEngines`](magistr_engine::NativeEngines) —
//! before the terminal is taken over, so a store that cannot be opened or a
//! refused lease is reported on a normal stdout and never inside the
//! alternate screen —
//! and [`EngineClient::subscribe`] attaches to the broadcast socket
//! `magistr mcp-serve` binds, decoding every line at that edge into the
//! typed [`NativeEvent`](magistr_engine::NativeEvent) and relaying it, with
//! how it was delivered, and every change of attachment onto the inbox as
//! [`Msg::Engine`](crate::cockpit::Msg::Engine); [`EngineClient::reseed`]
//! re-reads the store on a timer while nothing is attached and whenever the
//! feed asks, posting the typed [`NativeSnapshot`](magistr_engine::NativeSnapshot)
//! — or, when the feed asked and the store did not move, a confirmation
//! that it stands where the last snapshot left it.
//! [`EngineClient::admit_or_attach_config`] is the cockpit's own entry: when
//! another cockpit or loop holds the host role, this one attaches to that
//! host's owner endpoint and acts through its writer
//! ([`AttachedEnginePort`]); when it is the host, [`EngineClient::serve_owner`]
//! publishes the endpoint for the next one.

pub mod apply;
pub mod attached_port;
pub mod client;
pub mod error;
pub mod event;
pub mod feed;
pub mod feed_link;
pub mod native_port;
pub mod port;
pub mod reseed;

pub use apply::apply;
pub use attached_port::AttachedEnginePort;
pub use client::EngineClient;
pub use error::EngineError;
pub use event::EngineEvent;
pub use feed::spawn_feed;
pub use feed_link::FeedLink;
pub use native_port::NativeEnginePort;
pub use port::EnginePort;
pub use reseed::spawn_reseed;
