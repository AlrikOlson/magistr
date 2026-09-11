//! Native declared-command execution (chunk arch-14c9): the `run` use case.
//!
//! Admission stays [`crate::config::CommandAdmission`] — the merged
//! `[commands]` allowlist, the no-shell guard and the munging redirect are
//! decided before any executor is consulted and nothing here widens them.
//! Execution is one literal argv in the workspace root through a
//! [`RunExecutor`]; the production executor owns the child's dedicated
//! process group, bounds captured output, and turns a wall-clock timeout or a
//! cancellation into a killed group and a concrete terminal outcome. Output is
//! parsed, never dumped: the host-side diagnostic extractors first, then
//! plugin runner lanes, then the bundled runner parsers — each an open,
//! trust-ordered registry — and finally an honest head+tail cut with a
//! truncation manifest.
mod bounded_capture;
mod bundled_extractors;
mod bundled_runners;
mod cargo_diagnostics;
mod cargo_panic;
mod cargo_subcommand;
mod diagnostic;
mod diagnostic_extractor;
mod diagnostic_level;
mod head_tail;
mod plugin_runner_lane;
mod process_executor;
mod run_execution;
mod run_executor;
mod run_failure;
mod run_future;
mod run_input;
mod run_limits;
mod run_output;
mod run_parse;
mod run_report;
mod run_row;
mod run_service;
mod runner_bun;
mod runner_cargo_test;
mod runner_go;
mod runner_jest;
mod runner_nextest;
mod runner_parser;
mod runner_pytest;
mod runner_vitest;

pub use bundled_extractors::bundled_extractors;
pub use bundled_runners::bundled_runners;
pub use cargo_diagnostics::CargoDiagnostics;
pub use diagnostic::Diagnostic;
pub use diagnostic_extractor::DiagnosticExtractor;
pub use diagnostic_level::DiagnosticLevel;
pub use plugin_runner_lane::PluginRunnerLane;
pub use process_executor::ProcessExecutor;
pub use run_execution::RunExecution;
pub use run_executor::RunExecutor;
pub use run_failure::RunFailure;
pub use run_future::RunFuture;
pub use run_input::RunInput;
pub use run_limits::RunLimits;
pub use run_output::RunOutput;
pub use run_report::RunReport;
pub use run_row::RunRow;
pub use run_service::WorkspaceRun;
pub use runner_bun::BunRunner;
pub use runner_cargo_test::CargoTestRunner;
pub use runner_go::GoRunner;
pub use runner_jest::JestRunner;
pub use runner_nextest::NextestRunner;
pub use runner_parser::RunnerParser;
pub use runner_pytest::PytestRunner;
pub use runner_vitest::VitestRunner;
