//! Typed read projections. Revision, owner fencing and event position have
//! different authorities and must never be inferred from each other's values.

mod event_sequence;
mod owner_generation;
mod projection_freshness;
mod projection_port;
mod run_projection;
mod service;

pub use event_sequence::EventSequence;
pub use owner_generation::OwnerGeneration;
pub use projection_freshness::ProjectionFreshness;
pub use projection_port::ProjectionPort;
pub use run_projection::RunProjection;
pub use service::Projections;
