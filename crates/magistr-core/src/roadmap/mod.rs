//! Current roadmap backend selection and explicit YAML persistence.
//!
//! Shared project settings select the backend; the default is native
//! think-and-ship. An explicit YAML project uses `.magistr-roadmap.yml`.
//! All mutations go through [`YamlStore`] which owns a [`RoadmapIndex`]
//! for queries and mutations. No markdown is auto-generated — use
//! `magistr roadmap export` for explicit opt-in export.
//!
//! # Architecture
//!
//! - `schema.rs` — YAML-serializable types (`Roadmap`, `Phase`, `Task`)
//! - `index.rs` — In-memory query/mutation API (`RoadmapIndex`)
//! - `yaml_store.rs` — Persistence layer (load/save YAML atomically)
//! - `markdown_gen.rs` — YAML → Markdown renderer (opt-in export)
//!
//! # Usage
//!
//! ```ignore
//! use magistr_cli::roadmap::yaml_store::YamlStore;
//!
//! let store = YamlStore::load(".magistr-roadmap.yml")?;
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
pub use index::RoadmapIndex;
pub use phase::Phase;
pub use phase_status::PhaseStatus;
pub use schema::Roadmap;
pub use stats::RoadmapStats;
pub use task::Task;
pub use task_id::task_id;
pub use task_match::TaskMatch;
pub use yaml_path::yaml_path;
pub use yaml_store::YamlStore;
