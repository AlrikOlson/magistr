//! The plugin conformance kit (SDD §8.5): one programmatic entry point
//! produces a typed four-chapter report — interface, capabilities,
//! resources and the lang-pack fuzz corpus. The kit reads the plugin
//! directory with plain filesystem calls, deliberately: it runs against a
//! directory on the operator's machine, outside the workspace jail, and guest
//! execution is injected, so it runs identically over the real worker lane or
//! a scripted fake.
mod chapter;
mod chapter_id;
mod fixtures;
mod fuzz;
mod probe;
mod report;
mod run;

pub use chapter::TckChapter;
pub use chapter_id::TckChapterId;
pub use fuzz::TckFuzz;
pub use probe::TckProbe;
pub use report::TckReport;
pub use run::run_plugin_tck;
