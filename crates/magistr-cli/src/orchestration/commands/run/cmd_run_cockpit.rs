//! Run a live session on the terminal; the host owns project and provider lifetime.

use crate::agent;
use crate::application::InteractiveHost;
use crate::permissions::PermissionProfile;
use color_eyre::eyre::{self, Context as _};
use magistr_cockpit::cockpit::{Model, Size, StageTab, Summary};
use magistr_cockpit::theme::Theme;
use magistr_cockpit_runtime::channel::{Commands, Inbox};
use magistr_tui::runtime::{Options, Runtime};
use magistr_tui::term::RealTerminal;

/// Connect the configured session before entering the terminal event loop.
/// The shared host closes its provider and project connections after the view ends.
pub async fn cmd_run_cockpit(
    config: agent::AgentConfig,
    profile: PermissionProfile,
) -> eyre::Result<()> {
    let (inbox, messages) = Inbox::channel();
    let (sender, commands) = Commands::channel();
    let (host, model) = InteractiveHost::start(
        config.permission_profile(profile),
        Model::new(Size::new(80, 24))
            .with_tab(StageTab::Turn)
            .with_theme(Theme::from_os()),
        inbox.clone(),
        commands,
    )
    .await?;
    let terminal = match RealTerminal::stdio() {
        Ok(terminal) => terminal,
        Err(error) => {
            let cleanup = host.shutdown().await;
            let message = format!("Magistr could not open the terminal: {error}");
            return Err(eyre::eyre!(match cleanup {
                Ok(()) => message,
                Err(cleanup) =>
                    format!("{message}. The session could not close cleanly: {cleanup}"),
            }));
        }
    };
    // The process-wide interrupt closes the cockpit through its own leave
    // sequence; the host then drains the session it already owns.
    let runtime = Runtime::with_channels(
        terminal,
        Options::from_os().quit_on(crate::utils::shutdown::Shutdown::token()),
        inbox,
        messages,
        sender,
    );
    let model = runtime.run(model).await;
    let closed = host.shutdown().await;
    let model = model.context("the session ended unexpectedly")?;
    closed.map_err(|error| eyre::eyre!(error))?;
    println!("{}", Summary::of(&model));
    Ok(())
}
