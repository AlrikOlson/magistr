//! What the window does with what `update` answers.

use magistr_cockpit::cockpit::{Cmd, Model, Msg, update};
use tokio::sync::mpsc::UnboundedSender;

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
    commands: UnboundedSender<Cmd>,
}

impl Route {
    /// A route whose forwarded commands land on `commands`.
    #[must_use]
    pub fn new(commands: UnboundedSender<Cmd>) -> Self {
        Self { commands }
    }

    /// Fold `msg` into `model` and act on what the core answered.
    /// `true` when the core asked to quit.
    pub fn fold(&self, model: &mut Model, msg: Msg) -> bool {
        let mut quit = false;
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
                    let _ = self.commands.send(other);
                }
            }
        }
        quit
    }
}

#[cfg(test)]
#[path = "route.tests.rs"]
mod tests;
