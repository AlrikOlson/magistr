//! The host-side plumbing behind the served `tracker_*` tools: where this
//! project's tracker configuration is read from, how a provider's credential
//! is resolved and how its port is built.
//!
//! This lived in the standalone binary's CLI module until Magistr retired
//! that binary; the served `tracker_status` and `tracker_setup` tools were
//! always its real consumers, so it moved next to the tracker it serves.

pub mod build_port;
pub mod port;

pub use build_port::build_tracker_port_in;
pub(crate) use port::tracker_port_async;
pub use port::tracker_port_async_in;

/// Resolve the destination this project is configured to mirror into: the
/// data directory, the project id and the tracker configuration on disk.
pub(crate) fn tracker_config() -> (std::path::PathBuf, String, crate::tracker::TrackerConfig) {
    let data_dir = crate::infra::PersistenceConfig::from_env().data_dir;
    let project_id = crate::infra::resolve_project_id(None);
    let config = crate::tracker::config::load(&data_dir, &project_id);
    (data_dir, project_id, config)
}
