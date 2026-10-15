//! Desktop composition and current-executable child roles.
#![deny(dead_code, unused)]
// The same executable also serves pipe-based child roles; a GUI subsystem keeps
// ordinary Windows launches from opening a console while preserving those pipes.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod args;
mod attachments;
mod commands;
mod content;
mod conversations;
mod credit;
mod document;
mod drafts;
#[cfg(target_os = "macos")]
mod environment;
mod exit;
mod launch;
mod links;
mod preferences;
mod projects;
mod roadmap;
mod streams;
mod wire;
mod work;
mod workflow;

use args::{Child, Desktop};
use clap::Parser;
use color_eyre::eyre;

fn main() -> eyre::Result<()> {
    // Parse before starting any runtime, window or project owner. Worker argv is
    // exact; extra arguments must fail through the normal parser.
    let argv: Vec<_> = std::env::args_os().collect();
    let worker = args::is_worker(&argv);
    let cli = if worker {
        None
    } else {
        Some(Desktop::parse_from(argv))
    };
    #[cfg(target_os = "macos")]
    let environment_error = if cli.as_ref().is_some_and(|cli| cli.child.is_none()) {
        match environment::command_path() {
            Ok(path) => {
                // SAFETY: this is ordinary desktop startup on the main thread,
                // before Tokio, Tauri, logging or any other threads are started.
                // The shell reader has synchronously settled its child group.
                unsafe { std::env::set_var("PATH", path) };
                None
            }
            Err(error) => Some(error),
        }
    } else {
        None
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    if worker {
        return runtime
            .block_on(magistr_workspace::worker::serve_worker())
            .map_err(Into::into);
    }
    let cli = cli.expect("non-worker invocation was parsed");
    if let Some(child) = cli.child {
        return runtime.block_on(async {
            match child {
                Child::AcpAgent {
                    provider,
                    allow_tool,
                } => magistr::commands::misc::cmd_acp_agent(&provider, &allow_tool).await,
                Child::McpLaunch { target, dir } => {
                    magistr::commands::misc::cmd_mcp_launch(target, &dir).await
                }
            }
        });
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();
    #[cfg(target_os = "macos")]
    if let Some(error) = environment_error {
        tracing::warn!(%error, "Could not load the login shell command path; using the inherited path");
    }
    // block_on polls this future on the calling (main) thread. The native loop
    // stays here while the multithread scheduler services the admitted host.
    tauri::async_runtime::set(runtime.handle().clone());
    runtime.block_on(launch::run(cli))
}
