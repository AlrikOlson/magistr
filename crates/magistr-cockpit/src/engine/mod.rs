//! Semantic workflow events and ports consumed by the shared interface.
//! The CLI host owns store admission, live feeds and concrete port adapters.

pub mod apply;
pub mod describe;
pub mod event;
pub mod port;

pub use apply::apply;
pub use describe::describe;
pub use event::EngineEvent;
pub use port::EnginePort;
