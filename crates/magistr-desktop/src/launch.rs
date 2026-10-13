//! A native window selecting resident sessions owned by the Rust application.

use crate::args::Desktop;
use color_eyre::eyre;
use magistr::application::ProjectWindow;
use magistr_cockpit::cockpit::{Model, Size, StageTab};
use magistr_cockpit::theme::{MotionMode, Theme};
use magistr_gui2::Native;
use std::future::Future;

pub(crate) async fn run(cli: Desktop) -> eyre::Result<()> {
    let config = cli.config()?;
    let mut window = Native::new("Magistr");
    let commands = window
        .commands()
        .expect("new native window owns its command receiver");
    let mut theme = Theme::from_os();
    if cli.reduce_motion {
        theme = theme.with_motion(MotionMode::Reduced);
    }
    let model = Model::new(Size::new(80, 24))
        .with_tab(StageTab::Turn)
        .with_theme(theme);
    let configure = move |dir: &std::path::Path| cli.config_for(dir);
    let (host, model) =
        ProjectWindow::start(config, configure, model, window.inbox(), commands).await?;
    let result = window.run(model).map_err(eyre::Report::from);
    finish(result, host.shutdown()).await.map(|_| ())
}

async fn finish<T>(
    surface: eyre::Result<T>,
    shutdown: impl Future<Output = Result<(), String>>,
) -> eyre::Result<T> {
    match (surface, shutdown.await) {
        (Ok(model), Ok(())) => Ok(model),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(eyre::eyre!(error)),
        (Err(error), Err(cleanup)) => Err(eyre::eyre!(
            "The window could not finish: {error}. The session could not close: {cleanup}"
        )),
    }
}

#[cfg(test)]
#[path = "launch.tests.rs"]
mod tests;
