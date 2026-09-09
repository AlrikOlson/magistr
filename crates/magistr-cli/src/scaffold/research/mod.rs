//! Structured recommendation types produced by AI-assisted setup.
//!
//! `magistr_recommend` (during `init --interactive`) and headless init paths
//! parse the AI's output into `Recommendations`, which flows into the
//! scaffold pipeline as `ScaffoldSelections`.

mod recommendations;
pub use recommendations::{
    ConfidenceLevel, ConfidenceReport, Recommendations, RecommendedLifecycleHook,
};
mod generated_file;
pub use generated_file::GeneratedFile;
mod discovered_server;
pub use discovered_server::DiscoveredServer;
mod recommended_agent;
pub use recommended_agent::RecommendedAgent;
mod recommended_quality_gate;
pub use recommended_quality_gate::RecommendedQualityGate;
mod recommended_format;
pub use recommended_format::RecommendedFormat;
mod recommended_testing;
pub use recommended_testing::RecommendedTesting;
