//! One window over a succession of project hosts. The window keeps a single
//! inbox and command channel for its lifetime; each project it opens gets an
//! [`InteractiveHost`] of its own behind a fence, so a settled project can
//! never post into the next project's model.
//!
//! A switch admits the chosen project first and settles the previous host
//! only once that succeeded: a refusal — no Magistr setup, a held host lease
//! or workspace, a provider that would not start — leaves the current project
//! hosted and says why through [`Msg::CommandFailed`].

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use color_eyre::eyre;
use magistr_cockpit::cockpit::{Cmd, Model, Msg, ProjectSeed};
use magistr_cockpit::job::{JobId, JobOutput};
use magistr_cockpit_runtime::channel::{Commands, Inbox};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use super::InteractiveHost;
use super::interactive_host::combine;
use crate::agent::AgentConfig;

/// The home page's project list is one job per opened project. Its id sits
/// apart from the loop driver's gate jobs, which count up from zero and are
/// settled by id.
const PROJECTS_JOB: u64 = u64::MAX;

/// What an opener is handed for one project: where it is and the fenced
/// channels the project's host lives on.
pub(super) struct Opening {
    pub(super) dir: PathBuf,
    pub(super) inbox: Inbox,
    pub(super) commands: Commands,
    /// Whether the window already hosts a project, so this one must be owned
    /// outright rather than attached to.
    pub(super) switched: bool,
}

/// A project host the window can settle.
pub(super) trait Settle: Send + 'static {
    fn settle(self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
}

impl Settle for InteractiveHost {
    fn settle(self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        Box::pin(self.shutdown())
    }
}

type Opened<H> = Pin<Box<dyn Future<Output = Result<(H, ProjectSeed), String>> + Send>>;
pub(super) type Open<H> = Box<dyn FnMut(Opening) -> Opened<H> + Send>;

/// The window's session host across every project it opens.
///
/// Dropping this handle requests the same close as `shutdown`; keep the Tokio
/// runtime alive and await `shutdown` before exiting the process.
pub struct ProjectWindow {
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<(), String>>,
}

impl ProjectWindow {
    /// Open the first project with `first` and serve the window until it
    /// closes. `configure` builds the session configuration for a project the
    /// window switches to.
    ///
    /// # Errors
    /// As [`InteractiveHost::start`] for the first project.
    pub async fn start(
        first: AgentConfig,
        configure: impl Fn(&Path) -> eyre::Result<AgentConfig> + Send + 'static,
        model: Model,
        inbox: Inbox,
        commands: Commands,
    ) -> eyre::Result<(Self, Model)> {
        let dir = first.cwd.clone();
        let mut first = Some(first);
        let open: Open<InteractiveHost> = Box::new(move |opening: Opening| {
            let config = match first.take() {
                Some(config) => Ok(config),
                None => configure(&opening.dir),
            };
            Box::pin(async move {
                let config = config.map_err(|error| format!("{error:#}"))?;
                let inputs = config.inputs().clone();
                let Opening {
                    dir,
                    inbox,
                    commands,
                    switched,
                } = opening;
                let started = if switched {
                    InteractiveHost::open_owned(config, inbox.clone(), commands).await
                } else {
                    InteractiveHost::open(config, inbox.clone(), commands).await
                };
                let opened = started.map_err(|error| format!("{error:#}"))?;
                tokio::spawn(async move {
                    let listed = tokio::task::spawn_blocking(move || {
                        crate::commands::galaxy_data::collect(&dir, &inputs)
                    })
                    .await;
                    if let Ok(data) = listed {
                        inbox.send(Msg::Job {
                            id: JobId::new(PROJECTS_JOB),
                            output: JobOutput::Galaxy(Box::new(data)),
                        });
                    }
                });
                Ok(opened)
            })
        });
        let (window, seed) = Self::start_with(open, dir, inbox, commands)
            .await
            .map_err(|error| eyre::eyre!(error))?;
        Ok((window, seed.seed(model)))
    }

    pub(super) async fn start_with<H: Settle>(
        mut open: Open<H>,
        first: PathBuf,
        inbox: Inbox,
        commands: Commands,
    ) -> Result<(Self, ProjectSeed), String> {
        let (current, seeded) = Current::open(&mut open, first, false).await?;
        let (stop, stopped) = oneshot::channel();
        let serving = Serving {
            open,
            inbox,
            current,
            quit: false,
        };
        let task = tokio::spawn(serving.run(commands, stopped));
        Ok((
            Self {
                stop: Some(stop),
                task,
            },
            seeded,
        ))
    }

    /// Stop serving the window and settle the project it has open.
    ///
    /// # Errors
    /// The open project's host could not close cleanly.
    pub async fn shutdown(mut self) -> Result<(), String> {
        self.request_close();
        (&mut self.task)
            .await
            .map_err(|error| format!("The session could not close: {error}"))?
    }

    fn request_close(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

impl Drop for ProjectWindow {
    fn drop(&mut self) {
        self.request_close();
    }
}

/// One opened project: its host, the sender its commands go to and what it
/// has posted so far, not yet shown.
struct Current<H> {
    host: H,
    forward: UnboundedSender<Cmd>,
    posted: UnboundedReceiver<Msg>,
    /// Until the host hangs up its inbox.
    hearing: bool,
}

impl<H: Settle> Current<H> {
    async fn open(
        open: &mut Open<H>,
        dir: PathBuf,
        switched: bool,
    ) -> Result<(Self, ProjectSeed), String> {
        let (inbox, posted) = Inbox::channel();
        let (forward, commands) = Commands::channel();
        let (host, seeded) = open(Opening {
            dir,
            inbox,
            commands,
            switched,
        })
        .await?;
        Ok((
            Self {
                host,
                forward,
                posted,
                hearing: true,
            },
            seeded,
        ))
    }
}

struct Serving<H> {
    open: Open<H>,
    inbox: Inbox,
    current: Current<H>,
    /// A close asked for while a switch was draining the queue.
    quit: bool,
}

impl<H: Settle> Serving<H> {
    async fn run(
        mut self,
        mut commands: Commands,
        mut stopped: oneshot::Receiver<()>,
    ) -> Result<(), String> {
        let mut settled = Ok(());
        loop {
            tokio::select! {
                biased;
                _ = &mut stopped => break,
                posted = self.current.posted.recv(), if self.current.hearing => match posted {
                    Some(msg) => {
                        if !self.inbox.send(msg) {
                            break;
                        }
                    }
                    // The host ended by itself; its commands now fail visibly.
                    None => self.current.hearing = false,
                },
                command = commands.next() => match command {
                    None | Some(Cmd::Quit) => break,
                    Some(Cmd::OpenProject(dir)) => {
                        if let Some(previous) = self.switch(dir, &mut commands).await {
                            settled = combine(settled, previous);
                        }
                    }
                    Some(command) => self.forward(command),
                },
            }
            if self.quit {
                break;
            }
        }
        drop(commands);
        combine(settled, self.current.host.settle().await)
    }

    fn forward(&self, command: Cmd) {
        if let Err(error) = self.current.forward.send(command) {
            self.inbox.send(error.0.undelivered());
        }
    }

    /// Admit `dir`, then settle the project it replaces. `Some` carries how the
    /// previous host settled; `None` is a refusal that changed nothing.
    async fn switch(&mut self, dir: String, commands: &mut Commands) -> Option<Result<(), String>> {
        let refuse = |inbox: &Inbox, dir: String, error: String| {
            inbox.send(Msg::CommandFailed {
                cmd: Cmd::OpenProject(dir),
                error,
            });
        };
        let root = match prepared(Path::new(&dir)) {
            Ok(root) => root,
            Err(error) => {
                refuse(&self.inbox, dir, error);
                return None;
            }
        };
        let next = Current::open(&mut self.open, root, true).await;
        let (next, seeded) = match next {
            Ok(opened) => opened,
            Err(error) => {
                refuse(&self.inbox, dir, error);
                return None;
            }
        };
        // The fence: the previous project is no longer heard from this point,
        // so nothing it says while settling reaches the next project's model.
        let previous = std::mem::replace(&mut self.current, next);
        drop(previous.posted);
        drop(previous.forward);
        let settled = previous.host.settle().await;
        if let Err(error) = &settled {
            tracing::warn!(%error, "the previous project did not settle cleanly");
        }
        self.inbox.send(Msg::Rehosted(Box::new(seeded)));
        // What was asked of the previous project while the next one opened
        // goes back to its owner instead of to a project it was not meant for.
        while let Some(command) = commands.try_next() {
            if matches!(command, Cmd::Quit) {
                self.quit = true;
                break;
            }
            self.inbox.send(Msg::CommandFailed {
                cmd: command,
                error: "The project changed before this was sent.".into(),
            });
        }
        Some(settled)
    }
}

/// `dir` as a directory that already has its Magistr setup.
fn prepared(dir: &Path) -> Result<PathBuf, String> {
    let root = dir
        .canonicalize()
        .ok()
        .filter(|root| root.is_dir())
        .ok_or_else(|| format!("{} is not a directory.", dir.display()))?;
    if !crate::paths::project_json(&root)
        .map_err(|error| error.to_string())?
        .is_file()
    {
        return Err(format!(
            "{} has no Magistr setup yet. Run `magistr init` there, then open it again.",
            root.display()
        ));
    }
    Ok(root)
}

#[cfg(test)]
#[path = "project_window.tests.rs"]
mod tests;
