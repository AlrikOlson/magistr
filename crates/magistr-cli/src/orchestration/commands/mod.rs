//! Command handlers extracted from main.rs.
//!
//! Each submodule groups related command handlers.

use color_eyre::eyre;

use crate::agent;
use crate::cli;

pub mod archetypes;
pub mod chrome;
pub mod cockpit;
pub mod cockpit_roadmap;
pub mod config;
mod config_agent_set;
pub mod config_manage;
pub mod home;
pub(crate) mod home_data;
pub mod init;
pub mod misc;
pub mod models;
pub mod next;
pub mod preset;
pub mod run;
pub mod session;
pub mod session_started;
pub mod validate;
pub mod workbench_port;
pub mod workspace_files;

/// Convert CLI backend flag to agent backend, using auto-detection when needed.
///
/// `BackendFlag::Auto` detects the best available backend.
/// Explicit flags validate availability and feature compilation.
pub fn resolve_backend(
    flag: cli::BackendFlag,
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<agent::Backend> {
    flag.resolve(inputs).map_err(|msg| eyre::eyre!(msg))
}
