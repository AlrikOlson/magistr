//! Dependency-ordered batches across registered projects.
//!
//! [`ProjectGraph`] declares projects, dependencies, and the working tree
//! each project writes to. [`order`] rejects dependency cycles.
//!
//! [`BatchRun`] records released and skipped projects, deriving job status
//! from [`WorkQueue`]. A failed project skips its transitive dependents while
//! independent projects can continue. Projects sharing a working tree run
//! serially to avoid concurrent writes to the same Git index.
//!
//! These types schedule and track work. Execution requires a caller to claim
//! jobs, run them, and report results through the queue.
//!
//! [`WorkQueue`]: crate::fleet::WorkQueue
//! [`order`]: ProjectGraph::order

pub mod batch_row;
pub mod batch_run;
pub mod batch_state;
pub mod batch_status;
pub mod project_dep;
pub mod project_graph;

pub use batch_row::BatchRow;
pub use batch_run::BatchRun;
pub use batch_state::BatchState;
pub use batch_status::BatchStatus;
pub use project_dep::ProjectDep;
pub use project_graph::ProjectGraph;
