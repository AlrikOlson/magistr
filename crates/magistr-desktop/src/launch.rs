//! The production WebView observes the existing resident session owner and awaits it on every exit.
use crate::{args::Desktop, commands::DesktopState, streams::Streams};
use color_eyre::eyre;
use magistr::application::SessionRegistry;
use std::future::Future;

pub(crate) async fn run(cli: Desktop) -> eyre::Result<()> {
    let directory = cli.dir.clone();
    let inputs =
        magistr::config::inputs::ProjectInputs::from_env(directory.clone().unwrap_or_default());
    let session_inputs = inputs.clone();
    let reduce_motion = cli.reduce_motion;
    let mut initial = true;
    let registry = SessionRegistry::start(move |dir| {
        let config = cli.config_for(dir, &session_inputs, initial)?;
        initial = false;
        Ok(config)
    });
    let client = registry.client();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .menu(|app| {
            use tauri::menu::{Menu, MenuItem, Submenu};
            let quit = MenuItem::with_id(app, "quit", "Quit Magistr", true, Some("CmdOrCtrl+Q"))?;
            let application = Submenu::with_items(app, "Magistr", true, &[&quit])?;
            #[cfg(target_os = "macos")]
            {
                use tauri::menu::PredefinedMenuItem;
                let edit = Submenu::with_items(
                    app,
                    "Edit",
                    true,
                    &[
                        &PredefinedMenuItem::undo(app, None)?,
                        &PredefinedMenuItem::redo(app, None)?,
                        &PredefinedMenuItem::separator(app)?,
                        &PredefinedMenuItem::cut(app, None)?,
                        &PredefinedMenuItem::copy(app, None)?,
                        &PredefinedMenuItem::paste(app, None)?,
                        &PredefinedMenuItem::select_all(app, None)?,
                    ],
                )?;
                Menu::with_items(app, &[&application, &edit])
            }
            #[cfg(not(target_os = "macos"))]
            {
                Menu::with_items(app, &[&application])
            }
        })
        .on_menu_event(|app, event| {
            if event.id().as_ref() == "quit" {
                app.exit(0);
            }
        })
        .manage(DesktopState {
            client,
            streams: Streams::default(),
            directory,
            projects: std::sync::Arc::default(),
            inputs,
            reduce_motion,
        })
        .invoke_handler(tauri::generate_handler![
            crate::commands::overview,
            crate::commands::working_copies,
            crate::commands::pick_folder,
            crate::commands::preview_project,
            crate::commands::apply_setup,
            crate::commands::discard_setup,
            crate::commands::open_session,
            crate::commands::subscribe_session,
            crate::commands::subscribe_catalog,
            crate::commands::acknowledge,
            crate::commands::session_action,
            crate::commands::save_draft,
            crate::commands::close_session
        ])
        .build(context());
    let app = match app {
        Ok(app) => app,
        Err(error) => return finish::<()>(Err(eyre::eyre!(error)), registry.shutdown()).await,
    };
    let (settled, result) = tokio::sync::oneshot::channel();
    let mut closing = Some((registry, settled));
    // Native Quit must wait for application resources before the OS accepts exit.
    let exit = app.run_return(move |app, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event
            && let Some((registry, settled)) = closing.take()
        {
            tracing::info!("native Quit requested; awaiting session registry");
            api.prevent_exit();
            let app = app.clone();
            tokio::spawn(async move {
                let result = registry.shutdown().await;
                tracing::info!(
                    success = result.is_ok(),
                    "resident sessions settled before native exit"
                );
                let exit = i32::from(result.is_err());
                let _ = settled.send(result);
                app.exit(exit);
            });
        }
    });
    let shutdown = result
        .await
        .map_err(|_| "The window exited before sessions could settle.".to_owned());
    finish(
        if exit == 0 {
            Ok(())
        } else {
            Err(eyre::eyre!("The window exited with code {exit}."))
        },
        async { shutdown? },
    )
    .await
}
fn context() -> tauri::Context<tauri::Wry> {
    tauri::generate_context!()
}

async fn finish<T>(
    surface: eyre::Result<T>,
    shutdown: impl Future<Output = Result<(), String>>,
) -> eyre::Result<T> {
    match (surface, shutdown.await) {
        (Ok(value), Ok(())) => Ok(value),
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
