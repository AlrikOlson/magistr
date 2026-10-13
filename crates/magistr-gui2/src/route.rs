//! What the window does with what `update` answers.

use magistr_cockpit::cockpit::{Cmd, Model, Msg, update};
use tokio::sync::mpsc::Sender;

/// Route cockpit commands from the native event loop.
///
/// `Quit` ends the loop. `Animate` and `Still` require no separate scheduler because
/// each wait reads [`Model::animations`]. `Announce` is logged, and `Clipboard` writes
/// to the system clipboard.
///
/// Prompts, cancellation, permission answers, loop decisions, and engine requests go
/// through the commands channel to the session producers.
#[derive(Debug)]
pub struct Route {
    commands: Sender<Cmd>,
}

impl Route {
    /// A route whose forwarded commands land on `commands`.
    #[must_use]
    pub fn new(commands: Sender<Cmd>) -> Self {
        Self { commands }
    }

    /// Fold `msg` into `model` and act on what the core answered.
    /// `true` when the core asked to quit.
    pub fn fold(&self, model: &mut Model, msg: Msg) -> bool {
        let mut quit = false;
        let msg = match msg {
            Msg::Key(key)
                if key.is_press()
                    && key.code == magistr_cockpit::input::KeyCode::Char('v')
                    && key.mods == magistr_cockpit::input::Modifiers::SUPER =>
            {
                match arboard::Clipboard::new().and_then(|mut clip| clip.get_text()) {
                    Ok(text) => Msg::Paste(text),
                    Err(error) => {
                        tracing::warn!(target: "magistr-gui2", "Could not paste: {error}");
                        return false;
                    }
                }
            }
            other => other,
        };
        for cmd in update(model, msg) {
            match cmd {
                Cmd::Quit => quit = true,
                Cmd::Animate(_) | Cmd::Still => {}
                Cmd::Announce(text) => {
                    tracing::info!(target: "magistr-gui2", "{text}");
                }
                // The reader's clipboard is the window's own: the system clipboard,
                // best effort — a headless window logs what it could not hold.
                Cmd::Clipboard(text) => {
                    if let Err(e) =
                        arboard::Clipboard::new().and_then(|mut clip| clip.set_text(text))
                    {
                        tracing::warn!(target: "magistr-gui2", "clipboard: {e}");
                    }
                }
                other => {
                    // A receiver that is gone is a session that ended;
                    // the cockpit keeps painting, as it does on a
                    // terminal whose session hung up.
                    if let Err(error) = self.commands.try_send(other) {
                        let reason = match &error {
                            tokio::sync::mpsc::error::TrySendError::Full(_) => {
                                "The session is busy. Try the action again."
                            }
                            tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                                "The session connection closed. Reopen the session and try again."
                            }
                        };
                        self.fold(
                            model,
                            Msg::CommandFailed {
                                cmd: error.into_inner(),
                                error: reason.into(),
                            },
                        );
                    }
                }
            }
        }
        quit
    }
}

#[cfg(test)]
#[path = "route.tests.rs"]
mod tests;
