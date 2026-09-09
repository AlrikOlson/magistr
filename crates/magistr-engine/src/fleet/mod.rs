//! Fleet registration, policy distribution, and leased work.
//!
//! [`NodeHealth`] is derived from each [`FleetNode`]'s last renewal and the
//! fleet TTL. [`FleetRegistry::publish`] accepts a [`PolicyBundle`] only when
//! its generation increases and its digest matches. Nodes merge the bundle
//! with local rules; matching rules use the strictest effect.
//!
//! The digest detects accidental changes, not forgery. [`FleetStatus`]
//! reports policy drift but cannot prove which policy a node executes.
//!
//! [`WorkQueue`] places jobs on Ready nodes with free capacity and holds them
//! under claim leases. Expired claims are requeued with a new attempt number
//! that rejects stale results. [`StreamRelay`] orders each attempt's events
//! and exposes an append-only, cursor-based stream to clients.

pub mod fleet_node;
pub mod fleet_registry;
pub mod fleet_status;
pub mod job;
pub mod job_state;
pub mod node_health;
pub mod policy_bundle;
pub mod stream_relay;
pub mod work_queue;
pub mod work_unit;

pub use fleet_node::FleetNode;
pub use fleet_registry::FleetRegistry;
pub use fleet_status::FleetStatus;
pub use job::Job;
pub use job_state::JobState;
pub use node_health::NodeHealth;
pub use policy_bundle::PolicyBundle;
pub use stream_relay::StreamRelay;
pub use work_queue::WorkQueue;
pub use work_unit::WorkUnit;
