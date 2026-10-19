//! Desktop session choices and protocol-only child dispatch.

use clap::{Parser, Subcommand};
use color_eyre::eyre::{self, Context as _};
use magistr::agent::AgentConfig;
use magistr::commands::run::resolve_permission_profile;
use magistr::config::inputs::ProjectInputs;
use magistr::unified_mcp::Access;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
#[command(
    name = "magistr-desktop",
    version,
    about = "Open a project in Magistr",
    args_conflicts_with_subcommands = true
)]
pub(crate) struct Desktop {
    /// Message to send when the session opens
    pub prompt: Option<String>,
    /// Project folder to open when the window is ready
    #[arg(long)]
    pub dir: Option<PathBuf>,
    /// Agent provider (uses your configured default when omitted)
    #[arg(long)]
    pub provider: Option<String>,
    /// Model for this session
    #[arg(long)]
    pub model: Option<String>,
    /// Resume a saved conversation
    #[arg(long, conflicts_with = "continue_conversation")]
    pub resume: Option<String>,
    /// Continue the latest conversation
    #[arg(long = "continue")]
    pub continue_conversation: bool,
    /// Permission profile for this project
    #[arg(long)]
    pub trust: Option<String>,
    /// Reduce animated movement
    #[arg(long)]
    pub reduce_motion: bool,
    #[command(subcommand)]
    pub child: Option<Child>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Child {
    #[command(hide = true)]
    AcpAgent {
        #[arg(long)]
        provider: String,
        #[arg(long)]
        allow_tool: Vec<String>,
    },
    #[command(hide = true)]
    Mcp {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long = "config", value_name = "KEY=VALUE")]
        config: Vec<String>,
        #[arg(long, value_enum, default_value = "full")]
        access: Access,
    },
}

pub(crate) fn is_worker(argv: &[OsString]) -> bool {
    argv.len() == 2 && argv[1] == OsStr::new("--workspace-worker")
}

impl Desktop {
    fn session_choices(&self, config: AgentConfig) -> eyre::Result<AgentConfig> {
        Ok(config.with_run_args(
            self.model.as_deref(),
            self.resume.as_deref(),
            self.continue_conversation,
        )?)
    }

    /// Configure only an explicitly opened project, using the launch's captured
    /// inputs. Opening message and resume choices belong to the first session.
    pub(crate) fn config_for(
        &self,
        dir: &Path,
        inputs: &ProjectInputs,
        initial: bool,
    ) -> eyre::Result<AgentConfig> {
        let prompt = if initial {
            self.prompt.as_deref().unwrap_or("")
        } else {
            ""
        };
        let config = self.project(dir, prompt, inputs)?;
        if initial {
            self.session_choices(config)
        } else {
            Ok(config.with_run_args(self.model.as_deref(), None, false)?)
        }
    }

    fn project(
        &self,
        dir: &Path,
        prompt: &str,
        inputs: &ProjectInputs,
    ) -> eyre::Result<AgentConfig> {
        let root = dir
            .canonicalize()
            .wrap_err_with(|| format!("Could not open project {}", dir.display()))?;
        eyre::ensure!(
            root.is_dir(),
            "Project path is not a directory: {}",
            root.display()
        );
        let profile = resolve_permission_profile(&root, self.trust.as_deref())?;
        let mut config = AgentConfig::new(prompt, &root)
            .with_inputs(&inputs.for_project(&root))
            .with_project_context(&root)
            .permission_profile(profile);
        if let Some(provider) = &self.provider {
            config = config.provider(provider);
        }
        Ok(config)
    }
}

#[cfg(test)]
#[path = "args.tests.rs"]
mod tests;
