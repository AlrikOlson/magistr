//! Current plan backend selection and explicit YAML persistence.
//!
//! Shared project settings select the backend; the default is native
//! magistr. An explicit YAML project uses `.magistr-plan.yml`.
//! All mutations go through [`YamlStore`] which owns a [`PlanIndex`]
//! for queries and mutations. No markdown is auto-generated — use
//! `magistr plan export` for explicit opt-in export.
//!
//! # Architecture
//!
//! - `schema.rs` — YAML-serializable types (`Plan`, `Phase`, `Task`)
//! - `index.rs` — In-memory query/mutation API (`PlanIndex`)
//! - `yaml_store.rs` — Persistence layer (load/save YAML atomically)
//! - `markdown_gen.rs` — YAML → Markdown renderer (opt-in export)
//!
//! # Usage
//!
//! ```ignore
//! use magistr_cli::plan::yaml_store::YamlStore;
//!
//! let store = YamlStore::load(".magistr-plan.yml")?;
//! let stats = store.index().stats();
//! println!("{}/{} done", stats.checked_tasks, stats.total_tasks);
//! ```

pub mod backend_for;
mod ensure_file_export_allowed;
pub mod file_lock;
pub mod index;
pub mod markdown_gen;
pub mod phase;
pub mod phase_status;
pub mod schema;
pub mod stats;
pub mod task;
pub mod task_id;
pub mod task_match;
mod yaml_path;
pub mod yaml_store;

pub use backend_for::backend_for;
pub use ensure_file_export_allowed::ensure_file_export_allowed;
pub use index::InsertPosition;
pub use index::PlanIndex;
pub use phase::Phase;
pub use phase_status::PhaseStatus;
pub use schema::Plan;
pub use stats::PlanStats;
pub use task::Task;
pub use task_id::task_id;
pub use task_match::TaskMatch;
pub use yaml_path::yaml_path;
pub use yaml_store::YamlStore;
